//! The `--vw-*` design tokens from `packages/ui/src/theme.css`, in Rust.
//!
//! The colours are hand-mirrored; the `matches_theme_css` test fails if the two drift apart.

use egui::Color32;

/// One theme's colours. Field names follow the CSS tokens (`surface_2` is `--vw-surface-2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_2: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub fg: Color32,
    pub fg_muted: Color32,
    pub fg_subtle: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_active: Color32,
    pub accent_fg: Color32,
    pub accent_weak: Color32,
    pub danger: Color32,
    pub danger_weak: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const LIGHT: Palette = Palette {
    bg: hex(0xffffff),
    surface: hex(0xf8fafc),
    surface_2: hex(0xf1f5f9),
    border: hex(0xe2e8f0),
    border_strong: hex(0xcbd5e1),
    fg: hex(0x0f172a),
    fg_muted: hex(0x64748b),
    fg_subtle: hex(0x94a3b8),
    accent: hex(0x4a6eb0),
    accent_hover: hex(0x3f5e98),
    accent_active: hex(0x34507f),
    accent_fg: hex(0xffffff),
    accent_weak: hex(0xeef2fa),
    danger: hex(0xdc2626),
    danger_weak: hex(0xfef2f2),
};

pub const DARK: Palette = Palette {
    bg: hex(0x0f172a),
    surface: hex(0x131c31),
    surface_2: hex(0x1e293b),
    border: hex(0x293548),
    border_strong: hex(0x3b4a63),
    fg: hex(0xe8edf5),
    fg_muted: hex(0x94a3b8),
    fg_subtle: hex(0x64748b),
    accent: hex(0x6b8fd0),
    accent_hover: hex(0x89a6dc),
    accent_active: hex(0xa9c1e8),
    accent_fg: hex(0xffffff),
    accent_weak: hex(0x18243c),
    danger: hex(0xf87171),
    danger_weak: hex(0x2a1414),
};

// Radii (`--vw-radius-*`).
pub const RADIUS_SM: u8 = 6;
pub const RADIUS: u8 = 8;
pub const RADIUS_LG: u8 = 12;

// Spacing scale (`--vw-space-*`).
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;
pub const SPACE_6: f32 = 32.0;

// Type scale (`--vw-fs-*`).
pub const FS_XS: f32 = 11.0;
pub const FS_SM: f32 = 12.0;
pub const FS_MD: f32 = 13.0;
pub const FS_LG: f32 = 15.0;
pub const FS_XL: f32 = 20.0;

/// `--vw-control-h`.
pub const CONTROL_H: f32 = 32.0;
/// Height of `Toolbar.vue`.
pub const TOOLBAR_H: f32 = 48.0;

/// The palette for the theme `ui` is currently drawn in.
pub fn palette(ui: &egui::Ui) -> &'static Palette {
    if ui.visuals().dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEME_CSS: &str = include_str!("../../../packages/ui/src/theme.css");

    fn fields(p: &Palette) -> [(&'static str, Color32); 15] {
        [
            ("bg", p.bg),
            ("surface", p.surface),
            ("surface-2", p.surface_2),
            ("border", p.border),
            ("border-strong", p.border_strong),
            ("fg", p.fg),
            ("fg-muted", p.fg_muted),
            ("fg-subtle", p.fg_subtle),
            ("accent", p.accent),
            ("accent-hover", p.accent_hover),
            ("accent-active", p.accent_active),
            ("accent-fg", p.accent_fg),
            ("accent-weak", p.accent_weak),
            ("danger", p.danger),
            ("danger-weak", p.danger_weak),
        ]
    }

    /// The `--vw-<name>: #rrggbb;` declarations in the first `{ … }` block after `selector`.
    fn block_colors(selector: &str) -> Vec<(String, Color32)> {
        let start = THEME_CSS.find(selector).expect("selector in theme.css");
        let body = &THEME_CSS[start..];
        let body = &body[body.find('{').unwrap() + 1..body.find('}').unwrap()];
        body.lines()
            .filter_map(|line| {
                let (name, value) = line.trim().strip_prefix("--vw-")?.split_once(':')?;
                let hex = value.trim().trim_end_matches(';').strip_prefix('#')?;
                let rgb = u32::from_str_radix(hex, 16).ok()?;
                Some((name.to_owned(), super::hex(rgb)))
            })
            .collect()
    }

    fn assert_matches(selector: &str, palette: &Palette) {
        let css = block_colors(selector);
        for (name, color) in fields(palette) {
            let (_, expected) = css
                .iter()
                .find(|(n, _)| n == name)
                .unwrap_or_else(|| panic!("--vw-{name} missing from `{selector}`"));
            assert_eq!(color, *expected, "--vw-{name} in `{selector}`");
        }
    }

    #[test]
    fn matches_theme_css() {
        assert_matches(":root {", &LIGHT);
        assert_matches(":root[data-theme=\"dark\"]", &DARK);
    }
}
