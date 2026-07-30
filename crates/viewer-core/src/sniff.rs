//! File sniffing — the one piece of format-agnostic logic that was genuinely duplicated
//! across the apps at the Rust level (the three Tauri apps each hand-rolled an extension
//! check; image-shutter sniffs gzip magic bytes for `.svgz`).
//!
//! These are concrete, framework-free helpers. Extension/format *tables* (which extensions a
//! given app accepts) stay in the app — only the mechanism lives here.

use std::path::Path;

/// The file's extension, lowercased and without the leading dot; `None` if it has none.
///
/// ```
/// # use viewer_core::extension_lower;
/// assert_eq!(extension_lower("data.CSV").as_deref(), Some("csv"));
/// assert_eq!(extension_lower("noext"), None);
/// ```
pub fn extension_lower(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
}

/// True if the path's extension (case-insensitive) is one of `exts`, each given **without**
/// a leading dot (e.g. `&["geojson", "pmtiles", "geoparquet"]`).
///
/// ```
/// # use viewer_core::has_extension;
/// assert!(has_extension("map.GeoJSON", &["geojson", "pmtiles"]));
/// assert!(!has_extension("map.pdf", &["geojson", "pmtiles"]));
/// ```
pub fn has_extension(path: impl AsRef<Path>, exts: &[&str]) -> bool {
    match extension_lower(path) {
        Some(ext) => exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)),
        None => false,
    }
}

/// True if `bytes` begin with the gzip magic number (`1f 8b`) — used to transparently
/// handle gzip-compressed payloads such as `.svgz`.
///
/// ```
/// # use viewer_core::is_gzip;
/// assert!(is_gzip(&[0x1f, 0x8b, 0x08]));
/// assert!(!is_gzip(b"<svg"));
/// ```
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_lowercased_without_dot() {
        assert_eq!(
            extension_lower("a/b/File.PARQUET").as_deref(),
            Some("parquet")
        );
        assert_eq!(extension_lower("archive.tar.gz").as_deref(), Some("gz"));
        assert_eq!(extension_lower("README"), None);
    }

    #[test]
    fn has_extension_is_case_insensitive() {
        assert!(has_extension("x.pdf", &["pdf"]));
        assert!(has_extension("x.PDF", &["pdf"]));
        assert!(has_extension(
            "x.pmtiles",
            &["geojson", "pmtiles", "geoparquet"]
        ));
        assert!(!has_extension("x.csv", &["pdf"]));
        assert!(!has_extension("noext", &["pdf"]));
    }

    #[test]
    fn gzip_magic_detected() {
        assert!(is_gzip(&[0x1f, 0x8b]));
        assert!(!is_gzip(&[0x1f]));
        assert!(!is_gzip(b"<svg xmlns"));
    }
}
