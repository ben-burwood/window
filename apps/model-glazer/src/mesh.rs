//! Loading 3D model files into a single, render-ready triangle mesh.
//!
//! Every supported format (STL / OBJ / 3MF) is flattened into one [`Mesh`]: a
//! flat `positions` buffer (xyz triples) plus a `u32` index buffer, exactly the
//! shape the GL renderer uploads. Multi-object files are merged into that one
//! mesh — per-object structure is out of scope for a viewer. Normals are *not*
//! stored: the renderer derives face normals in the shader, so a facetted STL
//! and a smooth OBJ both light correctly without us reconstructing normals.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// An axis-aligned bounding box, used to frame the camera on load.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    /// The box enclosing a flat `[x, y, z, x, y, z, …]` position buffer, or
    /// `None` if it is empty.
    fn from_positions(positions: &[f32]) -> Option<Aabb> {
        if positions.len() < 3 {
            return None;
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in positions.chunks_exact(3) {
            for a in 0..3 {
                min[a] = min[a].min(p[a]);
                max[a] = max[a].max(p[a]);
            }
        }
        Some(Aabb { min, max })
    }

    pub fn center(&self) -> [f32; 3] {
        [
            0.5 * (self.min[0] + self.max[0]),
            0.5 * (self.min[1] + self.max[1]),
            0.5 * (self.min[2] + self.max[2]),
        ]
    }

    /// Radius of the bounding sphere (half the box diagonal). Never zero, so
    /// the camera always has a sane scale to work from.
    pub fn radius(&self) -> f32 {
        let dx = self.max[0] - self.min[0];
        let dy = self.max[1] - self.min[1];
        let dz = self.max[2] - self.min[2];
        let r = 0.5 * (dx * dx + dy * dy + dz * dz).sqrt();
        if r.is_finite() && r > 1e-6 {
            r
        } else {
            1.0
        }
    }
}

/// A render-ready triangle mesh: interleaved-free positions and 32-bit indices.
pub struct Mesh {
    /// Flat vertex positions: `[x0, y0, z0, x1, y1, z1, …]`.
    pub positions: Vec<f32>,
    /// Triangle indices into `positions` (3 per triangle).
    pub indices: Vec<u32>,
    /// Bounds of `positions`, for framing the camera.
    pub aabb: Aabb,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    fn build(positions: Vec<f32>, indices: Vec<u32>) -> Result<Mesh, String> {
        let aabb = Aabb::from_positions(&positions)
            .ok_or_else(|| "No drawable geometry in this file.".to_string())?;
        if indices.is_empty() {
            return Err("No drawable geometry in this file.".to_string());
        }
        Ok(Mesh {
            positions,
            indices,
            aabb,
        })
    }
}

/// Load a model file, dispatching on its extension. Returns a friendly error
/// string on an unsupported extension or a parse failure.
pub fn load(path: &Path) -> Result<Mesh, String> {
    match window_core::extension_lower(path).as_deref() {
        Some("stl") => load_stl(path),
        Some("obj") => load_obj(path),
        Some("3mf") => load_3mf(path),
        Some(other) => Err(format!("Unsupported file type: .{other}")),
        None => Err("File has no extension — cannot determine its type.".to_string()),
    }
}

fn load_stl(path: &Path) -> Result<Mesh, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let stl = stl_io::read_stl(&mut file).map_err(|e| format!("Not a valid STL: {e}"))?;

    let mut positions = Vec::with_capacity(stl.vertices.len() * 3);
    for v in &stl.vertices {
        positions.extend_from_slice(&[v[0], v[1], v[2]]);
    }
    let mut indices = Vec::with_capacity(stl.faces.len() * 3);
    for face in &stl.faces {
        indices.push(face.vertices[0] as u32);
        indices.push(face.vertices[1] as u32);
        indices.push(face.vertices[2] as u32);
    }
    Mesh::build(positions, indices)
}

fn load_obj(path: &Path) -> Result<Mesh, String> {
    // Triangulate n-gons and collapse to a single position index so the mesh
    // maps straight onto a GL vertex/index buffer. Materials/normals are
    // ignored — this is a geometry viewer.
    let opts = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, _materials) =
        tobj::load_obj(path, &opts).map_err(|e| format!("Not a valid OBJ: {e}"))?;

    let mut positions: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for model in &models {
        let m = &model.mesh;
        // Rebase this model's indices onto the merged position buffer.
        let base = (positions.len() / 3) as u32;
        positions.reserve(m.positions.len());
        indices.reserve(m.indices.len());
        positions.extend_from_slice(&m.positions);
        indices.extend(m.indices.iter().map(|&i| base + i));
    }
    Mesh::build(positions, indices)
}

fn load_3mf(path: &Path) -> Result<Mesh, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let models =
        threemf::read(BufReader::new(file)).map_err(|e| format!("Not a valid 3MF: {e:?}"))?;

    let mut positions: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    // Track whether the file is built purely from component assemblies so we can
    // tell the user *that* rather than the generic "no geometry" error.
    let mut saw_components = false;
    for model in &models {
        for object in &model.resources.object {
            match &object.object {
                threemf::model::ObjectData::Mesh(mesh) => {
                    let base = (positions.len() / 3) as u32;
                    positions.reserve(mesh.vertices.vertex.len() * 3);
                    indices.reserve(mesh.triangles.triangle.len() * 3);
                    for v in &mesh.vertices.vertex {
                        positions.extend_from_slice(&[v.x as f32, v.y as f32, v.z as f32]);
                    }
                    for t in &mesh.triangles.triangle {
                        indices.push(base + t.v1 as u32);
                        indices.push(base + t.v2 as u32);
                        indices.push(base + t.v3 as u32);
                    }
                }
                // Component objects place other objects via transforms to form an
                // assembly. We don't resolve that graph yet — concrete mesh
                // objects are rendered at their own coordinates and any
                // component transforms are dropped.
                threemf::model::ObjectData::Components { .. } => saw_components = true,
            }
        }
    }
    if indices.is_empty() && saw_components {
        return Err("This 3MF is a component assembly, which isn't supported yet.".to_string());
    }
    Mesh::build(positions, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    /// Write `contents` to a uniquely-named temp file with `ext` and return its path.
    fn temp_file(ext: &str, contents: &[u8]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("model_glazer_test_{}.{ext}", std::process::id()));
        std::fs::File::create(&path)
            .unwrap()
            .write_all(contents)
            .unwrap();
        path
    }

    #[test]
    fn loads_ascii_stl_triangle() {
        let stl = b"solid t\n\
             facet normal 0 0 1\n\
              outer loop\n\
               vertex 0 0 0\n\
               vertex 1 0 0\n\
               vertex 0 1 0\n\
              endloop\n\
             endfacet\n\
            endsolid t\n";
        let path = temp_file("stl", stl);
        let mesh = load(&path).expect("ASCII STL should load");
        std::fs::remove_file(&path).ok();

        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.aabb.min, [0.0, 0.0, 0.0]);
        assert_eq!(mesh.aabb.max, [1.0, 1.0, 0.0]);
    }

    #[test]
    fn loads_obj_triangle() {
        let obj = b"v 0 0 0\nv 2 0 0\nv 0 2 0\nf 1 2 3\n";
        let path = temp_file("obj", obj);
        let mesh = load(&path).expect("OBJ should load");
        std::fs::remove_file(&path).ok();

        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.aabb.max, [2.0, 2.0, 0.0]);
    }

    #[test]
    fn unsupported_extension_errors() {
        let path = temp_file("xyz", b"nope");
        let result = load(&path);
        std::fs::remove_file(&path).ok();
        match result {
            Err(e) => assert!(e.contains("Unsupported"), "unexpected error: {e}"),
            Ok(_) => panic!("expected an unsupported-extension error"),
        }
    }
}
