//! `viewer-core` — pure, framework-free logic shared across all four viewers.
//!
//! Contents are extracted in Phase 3 (file sniffing, path canonicalisation, a read-file
//! helper, and a shared error type). This crate must never depend on `tauri`, `eframe`,
//! `egui`, or `wry`; the invariant is checked by `just check-core-clean`.
