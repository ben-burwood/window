//! Shared egui chrome for the native viewers, so they look like the Tauri apps.
//!
//! [`tokens`] mirrors `packages/ui/src/theme.css`, [`install`] turns it into an egui theme, and
//! [`widgets`] are the Rust counterparts of the `@window/ui` Vue components.

pub mod theme;
pub mod tokens;
pub mod widgets;

pub use theme::install;
pub use tokens::{Palette, palette};
pub use widgets::{
    BadgeVariant, Toolbar, badge, empty_state, error_text, file_label, filter_field, meta_label,
    outdated_badge, primary_button, set_title, toggle_button,
};
