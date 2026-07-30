# 0002 — Shared release profile; `panic = "abort"` dropped

**Status:** accepted (Phase 2)

## Context
In a Cargo workspace, `[profile.*]` is honored **only** in the root manifest; per-crate
profiles are ignored (with a warning). `image-shutter` previously set its own
`[profile.release]` with `opt-level = 3`, `lto = "thin"`, `strip = true`, `panic = "abort"`.

## Decision
Adopt `image-shutter`'s release profile at the workspace root — **except `panic = "abort"`**,
which is intentionally omitted. A root-level `panic = "abort"` would apply to the three Tauri
apps as well, and Tauri/wry are not validated against `panic = abort` here (unwinding is the
default in the Tauri templates and some error paths may rely on it).

Root profile: `opt-level = 3`, `lto = "thin"`, `strip = true`, `codegen-units = 1`.

## Consequences
- `image-shutter`'s release binary now unwinds instead of aborting on panic — a negligible
  size/behavior difference for a viewer.
- All four apps share one consistent, optimized release profile.
- If we later want `panic = "abort"` for the egui app specifically, it cannot be expressed
  per-crate in a workspace; it would require splitting the profile another way.
