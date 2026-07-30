//! `viewer-core` — pure, framework-free logic shared across the viewers.
//!
//! This crate must never depend on `tauri`, `eframe`, `egui`, or `wry`; the invariant is
//! checked by `just check-core-clean`.
//!
//! It is intentionally small. The Phase 0 audit found that almost all viewer logic lives in
//! the apps' frontends (Vue/TS) or is framework-specific, so the only genuinely-duplicated,
//! format-agnostic Rust today is file sniffing. Following the project rule *"extract on the
//! second occurrence, not the first"*, single-occurrence utilities (path canonicalisation)
//! and not-yet-built features (recents, file-watch, PNG export) deliberately stay out until
//! a real second consumer appears — see `docs/decisions/0004-viewer-core-scope.md`.

pub mod sniff;

pub use sniff::{extension_lower, has_extension, is_gzip};
