//! DXF loading: parse a drawing once into a flat list of colored primitives in
//! world coordinates (Y-up), ready to paint directly with an `egui::Painter`.
//!
//! Curved entities (arcs, ellipses, splines, polyline bulges) are tessellated
//! into line segments here so the renderer only has to know about polylines,
//! circles and text. `INSERT` entities are expanded against their `BLOCK`
//! definition, recursing into nested inserts and carrying the insert's
//! transform and color down to the block's entities.

use std::collections::HashMap;
use std::path::Path;

use dxf::entities::{Entity, EntityType};
use dxf::Block;

use crate::palette::{self, Rgb};

/// How many nested `INSERT` levels to expand before giving up (guards against
/// blocks that reference themselves).
const MAX_BLOCK_DEPTH: u32 = 16;
/// Approximate arc/curve tessellation: one segment per this many degrees.
const DEG_PER_SEGMENT: f64 = 5.0;

/// A resolved color, or `None` meaning "foreground" (ACI 7 / by-layer-to-7 /
/// by-entity) — the renderer flips this to contrast the current background.
pub type Color = Option<Rgb>;

/// Axis-aligned bounds in world coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl Bounds {
    fn expand(&mut self, p: [f64; 2]) {
        self.min[0] = self.min[0].min(p[0]);
        self.min[1] = self.min[1].min(p[1]);
        self.max[0] = self.max[0].max(p[0]);
        self.max[1] = self.max[1].max(p[1]);
    }
    pub fn width(&self) -> f64 {
        self.max[0] - self.min[0]
    }
    pub fn height(&self) -> f64 {
        self.max[1] - self.min[1]
    }
    pub fn center(&self) -> [f64; 2] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
        ]
    }
}

/// A drawable primitive in world coordinates.
pub enum Primitive {
    /// A connected run of line segments. `closed` joins the last point to the first.
    Polyline {
        pts: Vec<[f64; 2]>,
        closed: bool,
        color: Color,
    },
    /// A true circle (only emitted when the transform keeps it circular).
    Circle {
        center: [f64; 2],
        r: f64,
        color: Color,
    },
    /// A single-line text label.
    Text {
        pos: [f64; 2],
        height: f64,
        angle_deg: f64,
        content: String,
        color: Color,
    },
}

/// A parsed DXF drawing reduced to primitives plus its extent.
pub struct Drawing {
    pub prims: Vec<Primitive>,
    /// `None` when the drawing is empty (nothing to fit to).
    pub bounds: Option<Bounds>,
}

// ---------------------------------------------------------------------------
// 2D affine transform (used to place block contents through nested inserts).
// Maps (x, y) -> (a*x + c*y + e, b*x + d*y + f).
// ---------------------------------------------------------------------------
#[derive(Clone, Copy)]
struct Xf {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Xf {
    const IDENTITY: Xf = Xf {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn apply(&self, x: f64, y: f64) -> [f64; 2] {
        [
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        ]
    }

    /// `self * other`: apply `other` first, then `self`.
    fn mul(&self, o: &Xf) -> Xf {
        Xf {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }

    fn translate(tx: f64, ty: f64) -> Xf {
        Xf {
            e: tx,
            f: ty,
            ..Xf::IDENTITY
        }
    }

    fn rotate(deg: f64) -> Xf {
        let (s, co) = deg.to_radians().sin_cos();
        Xf {
            a: co,
            b: s,
            c: -s,
            d: co,
            e: 0.0,
            f: 0.0,
        }
    }

    fn scale(sx: f64, sy: f64) -> Xf {
        Xf {
            a: sx,
            d: sy,
            ..Xf::IDENTITY
        }
    }

    /// True if the transform maps circles to circles (uniform scale + rotation
    /// or reflection, no shear). Returns the uniform radius scale factor if so.
    fn circle_scale(&self) -> Option<f64> {
        let col1 = self.a * self.a + self.b * self.b;
        let col2 = self.c * self.c + self.d * self.d;
        let dot = self.a * self.c + self.b * self.d;
        if (col1 - col2).abs() < 1e-9 * (col1 + col2 + 1.0) && dot.abs() < 1e-9 * (col1 + 1.0) {
            Some(col1.sqrt())
        } else {
            None
        }
    }

    /// Average linear scale (sqrt of the |determinant|) — used for text height.
    fn avg_scale(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    /// Rotation the transform applies, in degrees.
    fn rotation_deg(&self) -> f64 {
        self.b.atan2(self.a).to_degrees()
    }
}

/// Load and reduce a DXF file to drawable primitives.
pub fn load(path: &Path) -> Result<Drawing, String> {
    let src = dxf::Drawing::load_file(path).map_err(|e| format!("Invalid DXF: {e}"))?;

    // Layer name -> resolved color (None = foreground).
    let mut layer_colors: HashMap<String, Color> = HashMap::new();
    for layer in src.layers() {
        layer_colors.insert(layer.name.clone(), index_color(&layer.color));
    }

    // Block name -> definition, for INSERT expansion.
    let blocks: HashMap<String, &Block> = src.blocks().map(|b| (b.name.clone(), b)).collect();

    let ctx = Ctx {
        layer_colors,
        blocks,
    };

    let mut out = Vec::new();
    for e in src.entities() {
        emit(e, Xf::IDENTITY, None, &ctx, &mut out, 0);
    }

    let mut bounds: Option<Bounds> = None;
    for p in &out {
        accumulate_bounds(&mut bounds, p);
    }

    Ok(Drawing { prims: out, bounds })
}

struct Ctx<'a> {
    layer_colors: HashMap<String, Color>,
    blocks: HashMap<String, &'a Block>,
}

/// Resolve an entity's color given its layer and (for BYBLOCK) the enclosing
/// insert's color.
fn resolve_color(e: &Entity, ctx: &Ctx, parent: Color) -> Color {
    let color = &e.common.color;
    if color.index().is_some() {
        index_color(color)
    } else if color.is_by_block() {
        parent
    } else if color.is_by_layer() {
        ctx.layer_colors.get(&e.common.layer).copied().flatten()
    } else {
        None
    }
}

/// A layer/entity color that is only ever an index or defaults (used for the
/// layer table, where by-block/by-entity don't apply).
fn index_color(color: &dxf::Color) -> Color {
    match color.index() {
        Some(7) | None => None,
        Some(idx) => Some(palette::aci_rgb(idx)),
    }
}

fn emit(e: &Entity, xf: Xf, parent: Color, ctx: &Ctx, out: &mut Vec<Primitive>, depth: u32) {
    let color = resolve_color(e, ctx, parent);
    match &e.specific {
        EntityType::Line(l) => {
            push_poly(
                out,
                vec![xf.apply(l.p1.x, l.p1.y), xf.apply(l.p2.x, l.p2.y)],
                false,
                color,
            );
        }
        EntityType::Circle(c) => {
            if let Some(s) = xf.circle_scale() {
                out.push(Primitive::Circle {
                    center: xf.apply(c.center.x, c.center.y),
                    r: c.radius * s,
                    color,
                });
            } else {
                let pts = tessellate_arc(c.center.x, c.center.y, c.radius, 0.0, 360.0);
                push_poly(out, map_pts(&xf, &pts), true, color);
            }
        }
        EntityType::Arc(a) => {
            let pts = tessellate_arc(a.center.x, a.center.y, a.radius, a.start_angle, a.end_angle);
            push_poly(out, map_pts(&xf, &pts), false, color);
        }
        EntityType::Ellipse(el) => {
            let pts = tessellate_ellipse(el);
            let closed =
                (el.end_parameter - el.start_parameter).abs() >= std::f64::consts::TAU - 1e-6;
            push_poly(out, map_pts(&xf, &pts), closed, color);
        }
        EntityType::LwPolyline(p) => {
            let closed = p.flags & 1 != 0;
            let verts: Vec<(f64, f64, f64)> =
                p.vertices.iter().map(|v| (v.x, v.y, v.bulge)).collect();
            push_bulged(out, &xf, &verts, closed, color);
        }
        EntityType::Polyline(p) => {
            let closed = p.flags & 1 != 0;
            let verts: Vec<(f64, f64, f64)> = p
                .vertices()
                .map(|v| (v.location.x, v.location.y, v.bulge))
                .collect();
            push_bulged(out, &xf, &verts, closed, color);
        }
        EntityType::Spline(s) => {
            // A viewer-grade approximation: connect the fit points if present,
            // otherwise the control polygon.
            let src = if !s.fit_points.is_empty() {
                &s.fit_points
            } else {
                &s.control_points
            };
            let pts: Vec<[f64; 2]> = src.iter().map(|p| [p.x, p.y]).collect();
            if pts.len() >= 2 {
                push_poly(out, map_pts(&xf, &pts), false, color);
            }
        }
        EntityType::Text(t) => {
            push_text(
                out,
                xf.apply(t.location.x, t.location.y),
                t.text_height * xf.avg_scale(),
                t.rotation + xf.rotation_deg(),
                t.value.clone(),
                color,
            );
        }
        EntityType::MText(t) => {
            push_text(
                out,
                xf.apply(t.insertion_point.x, t.insertion_point.y),
                t.initial_text_height * xf.avg_scale(),
                t.rotation_angle + xf.rotation_deg(),
                mtext_plain(&t.text),
                color,
            );
        }
        EntityType::Insert(ins) => {
            if depth >= MAX_BLOCK_DEPTH {
                return;
            }
            let Some(block) = ctx.blocks.get(&ins.name) else {
                return;
            };
            // Transform placing block-space geometry into the insert's frame,
            // composed under the current transform.
            let local = Xf::translate(ins.location.x, ins.location.y)
                .mul(&Xf::rotate(ins.rotation))
                .mul(&Xf::scale(ins.x_scale_factor, ins.y_scale_factor))
                .mul(&Xf::translate(-block.base_point.x, -block.base_point.y));
            let child_xf = xf.mul(&local);
            for be in &block.entities {
                emit(be, child_xf, color, ctx, out, depth + 1);
            }
        }
        _ => {}
    }
}

fn push_poly(out: &mut Vec<Primitive>, pts: Vec<[f64; 2]>, closed: bool, color: Color) {
    if pts.len() >= 2 {
        out.push(Primitive::Polyline { pts, closed, color });
    }
}

/// Tessellate a bulge vertex list, map it through `xf`, and push the polyline.
fn push_bulged(
    out: &mut Vec<Primitive>,
    xf: &Xf,
    verts: &[(f64, f64, f64)],
    closed: bool,
    color: Color,
) {
    let pts = tessellate_bulged(verts, closed);
    push_poly(out, map_pts(xf, &pts), closed, color);
}

/// Push a text primitive, skipping blank content.
fn push_text(
    out: &mut Vec<Primitive>,
    pos: [f64; 2],
    height: f64,
    angle_deg: f64,
    content: String,
    color: Color,
) {
    if !content.trim().is_empty() {
        out.push(Primitive::Text {
            pos,
            height,
            angle_deg,
            content,
            color,
        });
    }
}

fn map_pts(xf: &Xf, pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    pts.iter().map(|p| xf.apply(p[0], p[1])).collect()
}

/// Points along an arc from `start_deg` to `end_deg` (CCW), inclusive of both ends.
fn tessellate_arc(cx: f64, cy: f64, r: f64, start_deg: f64, end_deg: f64) -> Vec<[f64; 2]> {
    let mut end = end_deg;
    if end <= start_deg {
        end += 360.0;
    }
    let sweep = end - start_deg;
    let segments = ((sweep / DEG_PER_SEGMENT).ceil() as usize).max(1);
    let mut pts = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let a = (start_deg + sweep * (i as f64 / segments as f64)).to_radians();
        pts.push([cx + r * a.cos(), cy + r * a.sin()]);
    }
    pts
}

fn tessellate_ellipse(el: &dxf::entities::Ellipse) -> Vec<[f64; 2]> {
    let cx = el.center.x;
    let cy = el.center.y;
    let mx = el.major_axis.x;
    let my = el.major_axis.y;
    // Minor axis is the major axis rotated 90° and scaled by the ratio.
    let nx = -my * el.minor_axis_ratio;
    let ny = mx * el.minor_axis_ratio;

    let mut end = el.end_parameter;
    if end <= el.start_parameter {
        end += std::f64::consts::TAU;
    }
    let sweep = end - el.start_parameter;
    let segments = ((sweep.to_degrees() / DEG_PER_SEGMENT).ceil() as usize).max(2);
    let mut pts = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = el.start_parameter + sweep * (i as f64 / segments as f64);
        let (s, c) = t.sin_cos();
        pts.push([cx + c * mx + s * nx, cy + c * my + s * ny]);
    }
    pts
}

/// Turn a vertex list (x, y, bulge) into a point list, expanding any bulge arcs.
/// A bulge is tan(sweep/4) of the arc from this vertex to the next.
fn tessellate_bulged(verts: &[(f64, f64, f64)], closed: bool) -> Vec<[f64; 2]> {
    if verts.is_empty() {
        return Vec::new();
    }
    let mut pts = Vec::new();
    let n = verts.len();
    let last = if closed { n } else { n - 1 };
    for i in 0..last {
        let (x0, y0, bulge) = verts[i];
        let (x1, y1, _) = verts[(i + 1) % n];
        pts.push([x0, y0]);
        if bulge.abs() > 1e-9 {
            arc_from_bulge(x0, y0, x1, y1, bulge, &mut pts);
        }
    }
    if !closed {
        let (xn, yn, _) = verts[n - 1];
        pts.push([xn, yn]);
    }
    pts
}

/// Append intermediate points of the bulge arc between (x0,y0) and (x1,y1).
/// The endpoints themselves are added by the caller.
fn arc_from_bulge(x0: f64, y0: f64, x1: f64, y1: f64, bulge: f64, pts: &mut Vec<[f64; 2]>) {
    let sweep = 4.0 * bulge.atan(); // signed included angle
    let chord = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
    if chord < 1e-12 {
        return;
    }
    let radius = chord / (2.0 * (sweep / 2.0).sin()).abs();
    // Midpoint of the chord, offset toward the arc center.
    let mxp = (x0 + x1) / 2.0;
    let myp = (y0 + y1) / 2.0;
    let dx = (x1 - x0) / chord;
    let dy = (y1 - y0) / chord;
    // Perpendicular; sign of bulge picks the side.
    let sagitta = radius - (radius * radius - (chord / 2.0).powi(2)).max(0.0).sqrt();
    let dir = if bulge > 0.0 { 1.0 } else { -1.0 };
    let cx = mxp - dy * dir * (radius - sagitta);
    let cy = myp + dx * dir * (radius - sagitta);

    let a0 = (y0 - cy).atan2(x0 - cx);
    let segments = ((sweep.abs().to_degrees() / DEG_PER_SEGMENT).ceil() as usize).max(1);
    for i in 1..segments {
        let a = a0 + sweep * (i as f64 / segments as f64);
        pts.push([cx + radius * a.cos(), cy + radius * a.sin()]);
    }
}

/// Strip the most common MTEXT inline formatting so the text is legible.
fn mtext_plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                match chars.peek() {
                    // Escaped literal brace/backslash.
                    Some('\\') | Some('{') | Some('}') => {
                        out.push(chars.next().unwrap());
                    }
                    // \P and \p are paragraph breaks; others are formatting codes.
                    Some('P') | Some('p') => {
                        chars.next();
                        out.push('\n');
                    }
                    _ => {
                        // Skip a formatting code up to its terminating ';' or letter group.
                        skip_mtext_code(&mut chars);
                    }
                }
            }
            '{' | '}' => {} // grouping — drop
            _ => out.push(c),
        }
    }
    out
}

fn skip_mtext_code(chars: &mut std::iter::Peekable<std::str::Chars>) {
    // Codes look like \fArial|...;  \C1;  \H2.5x;  — consume until ';'.
    for c in chars.by_ref() {
        if c == ';' {
            break;
        }
    }
}

fn accumulate_bounds(bounds: &mut Option<Bounds>, p: &Primitive) {
    let mut add = |pt: [f64; 2]| match bounds {
        Some(b) => b.expand(pt),
        None => *bounds = Some(Bounds { min: pt, max: pt }),
    };
    match p {
        Primitive::Polyline { pts, .. } => {
            for &pt in pts {
                add(pt);
            }
        }
        Primitive::Circle { center, r, .. } => {
            add([center[0] - r, center[1] - r]);
            add([center[0] + r, center[1] + r]);
        }
        Primitive::Text { pos, height, .. } => {
            add(*pos);
            add([pos[0], pos[1] + height]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(name: &str, content: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    /// A minimal ASCII DXF containing a single red LINE from (0,0) to (10,20).
    const LINE_DXF: &str = "\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n62\n1\n10\n0.0\n20\n0.0\n30\n0.0\n11\n10.0\n21\n20.0\n31\n0.0\n\
0\nENDSEC\n0\nEOF\n";

    #[test]
    fn loads_a_line_with_bounds_and_color() {
        let path = write_temp("drawing_paner_line.dxf", LINE_DXF);
        let d = load(&path).expect("should parse");
        assert_eq!(d.prims.len(), 1);
        let b = d.bounds.expect("has bounds");
        assert_eq!(b.min, [0.0, 0.0]);
        assert_eq!(b.max, [10.0, 20.0]);
        match &d.prims[0] {
            Primitive::Polyline { pts, color, .. } => {
                assert_eq!(pts.len(), 2);
                assert_eq!(*color, Some([255, 0, 0])); // ACI 1 = red
            }
            _ => panic!("expected a polyline"),
        }
    }

    #[test]
    fn rejects_garbage() {
        let path = write_temp("drawing_paner_bad.dxf", "this is not a dxf file");
        assert!(load(&path).is_err());
    }

    #[test]
    fn arc_tessellation_has_points_on_radius() {
        let pts = tessellate_arc(0.0, 0.0, 5.0, 0.0, 90.0);
        assert!(pts.len() >= 3);
        for p in &pts {
            let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!((r - 5.0).abs() < 1e-6);
        }
    }
}
