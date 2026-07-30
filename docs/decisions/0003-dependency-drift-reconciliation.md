# 0003 — Dependency drift reconciled in the workspace

**Status:** accepted (Phase 2)

## Context
The Phase 0 audit found small dependency drift across the apps that a single lockfile forces
us to address or consciously accept.

## Decisions
- **`geozero` → `0.15.1`.** data-framer pinned `0.14`, map-windower `0.15.1`; both use
  `default-features = false` + `with-wkb, with-geojson`. Unified to `0.15.1` in
  `[workspace.dependencies]`; data-framer bumped.
- **`tauri` `protocol-asset` feature** kept per-app: the shared workspace dep declares no
  features; map-windower and doc-viewer add `features = ["protocol-asset"]` on top (they use
  the asset protocol); data-framer does not.
- **Package version / authors normalized** via `[workspace.package]`: all crates are `0.1.0`,
  `authors = ["Ben Burwood"]` (data-framer's stale `0.2.0` / `authors = ["you"]` dropped).
- **Shared Tauri stack hoisted** to `[workspace.dependencies]`: `tauri`, `tauri-build`,
  `tauri-plugin-opener`, `tauri-plugin-dialog`, plus `serde`/`serde_json`. Format-specific
  deps (`polars`, `chrono`, `parquet`, `arrow-*`, `eframe`/`egui`/`resvg`/`usvg`/`tiny-skia`/
  `rfd`/`flate2`) stay in their app manifests.

## Consequences
- One root `Cargo.lock`; common dependencies compile once into the shared `target/`.
- geozero 0.14→0.15.1 required verifying data-framer's WKB/GeoJSON code still compiles
  (checked at the Phase 2 build gate).
