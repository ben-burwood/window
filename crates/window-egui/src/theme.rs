//! Turn the [`tokens`](crate::tokens) into an egui light and dark [`Style`].

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, Theme,
    Visuals, vec2,
};

use crate::tokens::{self, DARK, LIGHT};

/// Install the shared look on `ctx`. Call once from the app's `new`.
///
/// Both themes are registered, so egui keeps following the OS light/dark setting, like the
/// Tauri apps' `prefers-color-scheme` default.
pub fn install(ctx: &egui::Context) {
    for theme in [Theme::Light, Theme::Dark] {
        ctx.set_style_of(theme, style(theme));
    }
}

fn style(theme: Theme) -> Style {
    let mut style = Style {
        visuals: visuals(theme),
        ..Style::default()
    };

    let s = &mut style.spacing;
    s.item_spacing = vec2(tokens::SPACE_2, tokens::SPACE_1);
    // Slightly tighter than `.vw-btn`'s 12px, since egui buttons aren't fixed-height.
    s.button_padding = vec2(10.0, 3.0);
    // Lists and tables stay compact; the toolbar raises this to `CONTROL_H` for its controls.
    s.interact_size.y = 24.0;
    s.window_margin = Margin::same(tokens::SPACE_4 as i8);
    s.menu_margin = Margin::same(tokens::SPACE_1 as i8);

    let font = |size| FontId::new(size, FontFamily::Proportional);
    style.text_styles = [
        (TextStyle::Small, font(tokens::FS_XS)),
        (TextStyle::Body, font(tokens::FS_MD)),
        // Between `--vw-fs-sm` and `-md`: egui's default font reads small at 12px.
        (TextStyle::Button, font(12.5)),
        (TextStyle::Heading, font(tokens::FS_XL)),
        (
            TextStyle::Monospace,
            FontId::new(tokens::FS_SM, FontFamily::Monospace),
        ),
    ]
    .into();

    style
}

fn visuals(theme: Theme) -> Visuals {
    let dark = theme == Theme::Dark;
    let (p, mut v) = if dark {
        (&DARK, Visuals::dark())
    } else {
        (&LIGHT, Visuals::light())
    };

    let w = &mut v.widgets;
    // (state, fill, weak fill — what buttons use, border)
    for (state, bg, weak, border) in [
        // Labels, separators and panel borders.
        (&mut w.noninteractive, p.bg, p.surface, p.border),
        // `.vw-btn` at rest: page-coloured fill with a strong border.
        (&mut w.inactive, p.surface_2, p.bg, p.border_strong),
        (&mut w.hovered, p.surface_2, p.surface_2, p.fg_subtle),
        (&mut w.active, p.accent_weak, p.accent_weak, p.accent),
        (&mut w.open, p.surface_2, p.surface_2, p.border_strong),
    ] {
        state.bg_fill = bg;
        state.weak_bg_fill = weak;
        state.bg_stroke = Stroke::new(1.0, border);
        // Text stays the plain foreground in every state (`active` is also `strong_text_color`).
        state.fg_stroke = Stroke::new(1.0, p.fg);
        state.corner_radius = CornerRadius::same(tokens::RADIUS_SM);
        // CSS controls don't grow on hover.
        state.expansion = 0.0;
    }

    // Selected toggles and text selection: a tint of the accent, with accent-coloured text.
    v.selection.bg_fill = p.bg.lerp_to_gamma(p.accent, if dark { 0.35 } else { 0.22 });
    v.selection.stroke = Stroke::new(1.0, p.accent_active);

    v.weak_text_color = Some(p.fg_muted);
    v.hyperlink_color = p.accent;
    v.faint_bg_color = p.surface;
    v.extreme_bg_color = p.surface;
    v.text_edit_bg_color = Some(p.bg);
    v.code_bg_color = p.surface_2;
    v.warn_fg_color = p.danger;
    v.error_fg_color = p.danger;
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);

    v.panel_fill = p.bg;
    v.window_fill = p.bg;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(tokens::RADIUS_LG);
    v.menu_corner_radius = CornerRadius::same(tokens::RADIUS);
    // `--vw-shadow`.
    let shadow = if dark {
        Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(115),
        }
    } else {
        Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(22),
        }
    };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;

    v
}
