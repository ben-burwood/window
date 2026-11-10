//! Syntax highlighting for the source editor, via syntect.
//!
//! syntect's bundled syntaxes have no TOML, so the syntax set comes from `two-face` (bat's
//! extended set); the themes stay syntect's defaults, which is what `CodeTheme` picks from.

use egui::text::LayoutJob;
use egui_extras::syntax_highlighting::{self, CodeTheme, SyntectSettings};
use std::sync::LazyLock;

use crate::config::Format;

/// Loaded once: egui_extras memoizes highlighting by the settings' address, so it must be stable.
static SETTINGS: LazyLock<SyntectSettings> = LazyLock::new(|| SyntectSettings {
    ps: two_face::syntax::extra_newlines(),
    ts: syntect::highlighting::ThemeSet::load_defaults(),
});

pub fn highlight(
    ctx: &egui::Context,
    style: &egui::Style,
    theme: &CodeTheme,
    code: &str,
    format: Format,
) -> LayoutJob {
    syntax_highlighting::highlight_with(ctx, style, theme, code, format.extension(), &SETTINGS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_format_has_a_syntax() {
        for format in Format::ALL {
            assert!(
                SETTINGS
                    .ps
                    .find_syntax_by_extension(format.extension())
                    .is_some(),
                "no syntax for {}",
                format.name()
            );
        }
    }
}
