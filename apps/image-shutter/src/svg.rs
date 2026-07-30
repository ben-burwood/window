//! SVG loading and rasterization.
//!
//! An SVG is parsed once into a [`usvg::Tree`]; the tree is then rasterized on
//! demand at whatever pixel scale the viewer currently needs (so the image
//! stays crisp when zoomed) into an [`egui::ColorImage`] we can upload as a
//! texture.

use egui::ColorImage;
use std::io::Read;
use std::path::Path;

/// Largest texture dimension we will rasterize to, in pixels. Caps memory use
/// at extreme zoom levels; egui simply scales the existing texture beyond this.
const MAX_DIM: u32 = 8192;

/// A parsed SVG plus its intrinsic (unscaled) pixel size.
pub struct SvgImage {
    tree: usvg::Tree,
    /// Intrinsic width in px (rounded up to a whole pixel).
    pub width: u32,
    /// Intrinsic height in px.
    pub height: u32,
}

/// Load and parse an SVG (or gzipped `.svgz`) from disk.
pub fn load(path: &Path) -> Result<SvgImage, String> {
    let raw = std::fs::read(path).map_err(|e| format!("Could not read file: {e}"))?;
    let data = maybe_gunzip(raw);

    let mut opt = usvg::Options {
        // Lets the SVG resolve relative `href`s (e.g. embedded raster images).
        resources_dir: std::fs::canonicalize(path)
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf())),
        ..usvg::Options::default()
    };
    opt.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("Invalid SVG: {e}"))?;

    let size = tree.size().to_int_size();
    Ok(SvgImage {
        tree,
        width: size.width().max(1),
        height: size.height().max(1),
    })
}

/// Rasterize the SVG at `scale` (1.0 == intrinsic size) into a premultiplied
/// RGBA image ready to upload as an egui texture.
pub fn render(img: &SvgImage, scale: f32) -> ColorImage {
    let scale = scale.max(0.001);
    let w = (((img.width as f32) * scale).round() as u32)
        .clamp(1, MAX_DIM);
    let h = (((img.height as f32) * scale).round() as u32)
        .clamp(1, MAX_DIM);

    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .expect("failed to allocate pixmap");

    // Use the *actual* per-axis scale after clamping/rounding so the render
    // fills the pixmap exactly.
    let transform = tiny_skia::Transform::from_scale(
        w as f32 / img.width as f32,
        h as f32 / img.height as f32,
    );
    resvg::render(&img.tree, transform, &mut pixmap.as_mut());

    // tiny-skia's buffer is premultiplied RGBA, which is exactly what egui wants.
    ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data())
}

/// Transparently decompress gzip-compressed SVG (`.svgz`); returns the input
/// unchanged if it is not gzipped.
fn maybe_gunzip(data: Vec<u8>) -> Vec<u8> {
    const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
    if data.len() >= 2 && data[0..2] == GZIP_MAGIC {
        let mut decoder = flate2::read::GzDecoder::new(&data[..]);
        let mut out = Vec::new();
        if decoder.read_to_end(&mut out).is_ok() {
            return out;
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A red rectangle on a fully transparent 100x50 canvas.
    const SAMPLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
        <rect x="10" y="10" width="30" height="30" fill="#ff0000"/></svg>"##;

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        path
    }

    #[test]
    fn loads_intrinsic_size() {
        let path = write_temp("image_shutter_test.svg", SAMPLE.as_bytes());
        let img = load(&path).expect("should parse");
        assert_eq!((img.width, img.height), (100, 50));
    }

    #[test]
    fn renders_scaled_and_keeps_transparency() {
        let path = write_temp("image_shutter_test2.svg", SAMPLE.as_bytes());
        let img = load(&path).unwrap();

        // Render at 2x -> 200x100 px.
        let ci = render(&img, 2.0);
        assert_eq!(ci.size, [200, 100]);

        // A pixel inside the rect (scaled) should be opaque red...
        let px = ci.pixels[(20 * 200 + 40) as usize];
        assert!(px.a() > 200 && px.r() > 200 && px.g() < 60 && px.b() < 60);

        // ...and a corner pixel should be fully transparent.
        let corner = ci.pixels[0];
        assert_eq!(corner.a(), 0);
    }

    #[test]
    fn rejects_garbage() {
        let path = write_temp("image_shutter_bad.svg", b"this is not svg");
        assert!(load(&path).is_err());
    }
}
