//! The Drawing Paner application: window chrome, input handling (zoom / pan /
//! fit), a light/dark background toggle, and direct vector rendering of a DXF's
//! primitives with an `egui::Painter`.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::dxf::{self, Bounds, Primitive};

/// Screen pixels per world unit. Fit picks a value inside this band; the zoom
/// controls keep it there.
const MIN_ZOOM: f32 = 1e-6;
const MAX_ZOOM: f32 = 1e7;
/// Multiplicative step for the discrete zoom controls (buttons / keys).
const ZOOM_STEP: f32 = 1.25;
/// Text below this on-screen height is skipped (unreadable), above it clamped.
const MIN_TEXT_PX: f32 = 4.0;
const MAX_TEXT_PX: f32 = 4000.0;

pub struct DrawingPanerApp {
    drawing: Option<dxf::Drawing>,
    file_name: String,
    error: Option<String>,
    /// Screen pixels per world unit.
    zoom: f32,
    /// Pan offset (screen points) relative to a drawing centered in the panel.
    offset: Vec2,
    /// Fit-to-window is deferred to the first frame after a load, when the
    /// panel size is known.
    fit_pending: bool,
    /// When true, paint a dark background (CAD-style); otherwise light.
    dark_bg: bool,
    /// Path of the loaded file, used to reload.
    current_path: Option<PathBuf>,
    /// Set by the file watcher (on its own thread) when the file changes on disk.
    outdated: Arc<AtomicBool>,
    /// Live file watcher; replaced on each load, dropped on exit.
    _watch: Option<window_core::FileWatch>,
    /// Wakes the UI from the watcher thread.
    egui_ctx: egui::Context,
}

impl DrawingPanerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        window_egui::install(&cc.egui_ctx);
        let mut app = Self {
            drawing: None,
            file_name: String::new(),
            error: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            fit_pending: false,
            dark_bg: false,
            current_path: None,
            outdated: Arc::new(AtomicBool::new(false)),
            _watch: None,
            egui_ctx: cc.egui_ctx.clone(),
        };
        if let Some(path) = initial_file {
            app.load(&path);
        }
        app
    }

    fn load(&mut self, path: &Path) {
        self.outdated.store(false, Ordering::Relaxed);
        self.file_name = window_core::file_name(path);
        match dxf::load(path) {
            Ok(drawing) => {
                self.drawing = Some(drawing);
                self.error = None;
                self.zoom = 1.0;
                self.offset = Vec2::ZERO;
                self.fit_pending = true;
                self.current_path = Some(path.to_path_buf());
                self.start_watch(path);
            }
            Err(e) => {
                self.error = Some(e);
                self.drawing = None;
                self.current_path = None;
                self._watch = None;
            }
        }
    }

    fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn start_watch(&mut self, path: &Path) {
        let outdated = self.outdated.clone();
        let ctx = self.egui_ctx.clone();
        self._watch = window_core::watch_file(path, Duration::from_millis(300), move || {
            outdated.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        })
        .ok();
    }

    fn reload(&mut self) {
        if let Some(path) = self.current_path.clone() {
            self.load(&path);
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DXF drawing", &["dxf"])
            .pick_file()
        {
            self.load(&path);
        }
    }
}

impl eframe::App for DrawingPanerApp {
    // egui 0.35: the app fills a root `Ui`; we nest our own panels inside it.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        window_egui::set_title(&ctx, "Drawing Paner", &self.file_name, false);
        self.handle_shortcuts(&ctx);

        if self.drawing.is_some() {
            window_egui::Toolbar::show(ui, |bar| {
                bar.start(|ui| {
                    window_egui::file_label(ui, &self.file_name);
                    if self.outdated.load(Ordering::Relaxed)
                        && window_egui::outdated_badge(ui, false)
                    {
                        self.reload();
                    }
                });
                // Zoom controls. Clicking the percentage fits the drawing.
                bar.center(|ui| {
                    if ui.button("Fit").clicked() {
                        self.fit_pending = true;
                    }
                    if ui.button("−").clicked() {
                        self.zoom_by(1.0 / ZOOM_STEP);
                    }
                    if ui
                        .button(format!("{:.0}%", self.zoom * 100.0))
                        .on_hover_text("Fit to window")
                        .clicked()
                    {
                        self.fit_pending = true;
                    }
                    if ui.button("+").clicked() {
                        self.zoom_by(ZOOM_STEP);
                    }
                });
                bar.end(|ui| {
                    if ui.button("Open").clicked() {
                        self.open_dialog();
                    }
                    window_egui::toggle_button(ui, &mut self.dark_bg, "Dark")
                        .on_hover_text("Use a dark background");
                });
            });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.drawing.is_none() {
                let open = window_egui::empty_state(
                    ui,
                    "Open a drawing",
                    "Open a DXF file to get started",
                    self.error.as_deref(),
                );
                if open {
                    self.open_dialog();
                }
                return;
            }
            self.show_canvas(ui, &ctx);
        });
    }
}

impl DrawingPanerApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // rfd opens a native modal, so run it outside the input closure.
        let open = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
        if open {
            self.open_dialog();
        }
        if self.drawing.is_some() {
            let (fit, zoom_in, zoom_out) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::F) || i.key_pressed(egui::Key::Num0),
                    i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                    i.key_pressed(egui::Key::Minus),
                )
            });
            if fit {
                self.fit_pending = true;
            }
            if zoom_in {
                self.zoom_by(ZOOM_STEP);
            }
            if zoom_out {
                self.zoom_by(1.0 / ZOOM_STEP);
            }
        }
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let content = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(content, Sense::click_and_drag());

        let p = if self.dark_bg {
            &window_egui::tokens::DARK
        } else {
            &window_egui::tokens::LIGHT
        };
        let (bg, fg) = (p.bg, p.fg);
        let painter = ui.painter_at(content);
        painter.rect_filled(content, 0.0, bg);

        let drawing = self.drawing.as_ref().unwrap();
        let Some(bounds) = drawing.bounds else {
            painter.text(
                content.center(),
                Align2::CENTER_CENTER,
                "No drawable geometry in this file.",
                FontId::proportional(16.0),
                fg,
            );
            return;
        };

        // Fit-to-window once the panel size is known.
        if self.fit_pending {
            self.zoom = fit_zoom(&bounds, content.size());
            self.offset = Vec2::ZERO;
            self.fit_pending = false;
        }

        let pc = content.center();

        // Zoom to cursor. Plain mouse-wheel scroll zooms; Ctrl+scroll and
        // trackpad pinch come through as `zoom_delta`.
        let (scroll, zoom_delta) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = (scroll * 0.0015).exp() * zoom_delta;
        if factor != 1.0
            && let Some(cursor) = response.hover_pos()
        {
            let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            let ratio = new_zoom / self.zoom;
            // Keep the world point under the cursor fixed.
            self.offset -= (cursor - pc - self.offset) * (ratio - 1.0);
            self.zoom = new_zoom;
        }

        // Pan on drag.
        if response.dragged() {
            self.offset += response.drag_delta();
        }

        // World (Y-up) -> screen (Y-down), centered on the drawing's bounds.
        let z = self.zoom as f64;
        let wc = bounds.center();
        let ox = (pc.x + self.offset.x) as f64;
        let oy = (pc.y + self.offset.y) as f64;
        let w2s = |p: [f64; 2]| -> Pos2 {
            Pos2::new(
                (ox + (p[0] - wc[0]) * z) as f32,
                (oy - (p[1] - wc[1]) * z) as f32,
            )
        };

        for prim in &drawing.prims {
            match prim {
                Primitive::Polyline { pts, closed, color } => {
                    let mut sp: Vec<Pos2> = pts.iter().map(|&p| w2s(p)).collect();
                    if *closed && sp.len() >= 2 {
                        sp.push(sp[0]);
                    }
                    if polyline_visible(&sp, content) {
                        painter.add(egui::Shape::line(sp, Stroke::new(1.0, pick(*color, fg))));
                    }
                }
                Primitive::Circle { center, r, color } => {
                    let c = w2s(*center);
                    let rr = (*r * z) as f32;
                    if rr >= 0.05
                        && content.intersects(Rect::from_center_size(c, Vec2::splat(rr * 2.0)))
                    {
                        painter.circle_stroke(c, rr, Stroke::new(1.0, pick(*color, fg)));
                    }
                }
                Primitive::Text {
                    pos,
                    height,
                    angle_deg,
                    content: text,
                    color,
                } => {
                    let size = ((*height * z) as f32).min(MAX_TEXT_PX);
                    let p = w2s(*pos);
                    if size >= MIN_TEXT_PX && content.expand(size * 4.0).contains(p) {
                        let col = pick(*color, fg);
                        let galley =
                            painter.layout_no_wrap(text.clone(), FontId::proportional(size), col);
                        // DXF text sits above its baseline; nudge the top-left
                        // anchor up so it reads at the insertion point. World is
                        // Y-up and CCW-positive, which is clockwise on screen —
                        // exactly TextShape's rotation convention.
                        let anchor = Pos2::new(p.x, p.y - galley.size().y);
                        let mut shape = egui::epaint::TextShape::new(anchor, galley, col);
                        shape.angle = (*angle_deg as f32).to_radians();
                        painter.add(shape);
                    }
                }
            }
        }
    }
}

/// Resolve a primitive color, falling back to the foreground color.
fn pick(color: dxf::Color, fg: Color32) -> Color32 {
    match color {
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
        None => fg,
    }
}

/// Zoom (px/world-unit) that fits `bounds` inside `panel` with a small margin.
fn fit_zoom(bounds: &Bounds, panel: Vec2) -> f32 {
    let w = bounds.width();
    let h = bounds.height();
    let sx = if w > 1e-9 {
        panel.x as f64 / w
    } else {
        f64::INFINITY
    };
    let sy = if h > 1e-9 {
        panel.y as f64 / h
    } else {
        f64::INFINITY
    };
    let z = sx.min(sy);
    if z.is_finite() && z > 0.0 {
        (z as f32 * 0.98).clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        // Degenerate (single point / empty extent): keep a sane default.
        1.0
    }
}

/// True if the polyline's screen bounding box intersects the visible panel.
fn polyline_visible(pts: &[Pos2], clip: Rect) -> bool {
    pts.len() >= 2 && clip.intersects(Rect::from_points(pts))
}
