# 0007 — Tauri config consolidation via `tauri.base.json`

**Status:** accepted (Phase 6)

## Context
The three Tauri apps' `tauri.conf.json` files were ~90% identical. Tauri v2 (verified against
the 2.11 docs) merges a `--config <file>` over the main config using **RFC 7396 JSON Merge
Patch**: the provided file wins, nested objects merge recursively, **arrays replace wholesale**.
The docs do **not** specify how relative paths in a split config are anchored.

## Decisions
- **`tauri.base.json` at the repo root holds only non-path, globally-uniform keys**: the
  `build` commands (`beforeDevCommand`/`beforeBuildCommand`/`devUrl`) and
  `bundle.active`/`targets`/`publisher`/`category`/`copyright` (the last three net-new). It is
  merged over each app via `--config`.
- **Path-bearing keys stay in each app's `tauri.conf.json`** — `version: "../package.json"`,
  `build.frontendDist: "../dist"`, and `bundle.icon` (`icons/…`). Because the docs don't
  guarantee where a base file's relative paths resolve, keeping them app-side (anchored at
  `src-tauri`, as always) removes all ambiguity. Also per-app: `productName`, `identifier`,
  `app.windows`, `bundle.fileAssociations`, `app.security.assetProtocol`.
- **Merge is wired into `justfile`**: `just bundle <app>` / `just tauri-dev <app>` always pass
  `--config ../../tauri.base.json`. Building a Tauri app without the base omits the shared
  metadata; the release workflow (Phase 7) uses the same flag.
- **CSP left `null` (deferred).** A strict CSP can't be uniform: map-windower loads remote OSM
  raster tiles, so it needs origins the others don't — and since the base wins under RFC 7396,
  a base CSP would override any per-app allowance. Hardening CSP needs per-app values plus
  runtime testing (workers/blob for PDF.js, MapLibre) that the headless build can't do. Tracked
  as a follow-up.
- **`assetProtocol` scope kept `["**"]` per app (deferred).** These viewers open files from
  arbitrary locations; statically narrowing the scope would break that. Correct tightening
  needs runtime per-file scope granting — deferred with the other runtime features.

## Consequences
- Effective (merged) config verified by RFC-7396 simulation to equal the original plus the new
  shared metadata; all configs are valid JSON. Per-app configs shrank to per-app content
  (residual length is icon paths + fileAssociations, both irreducible).
- `cargo build` (no `--config`) compiles against the slimmed app config (missing shared bundle
  metadata, which only matters at CLI bundle time) — still valid for `generate_context!`.
- The full "merged bundle has correct associations/publisher/CSP" gate requires running
  `just bundle <app>` on each platform, which the headless environment can't do.
