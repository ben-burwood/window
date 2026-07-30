# 0005 — One neutral design-token vocabulary (`--vw-*`)

**Status:** accepted (Phase 5)

## Context
The Phase 0 audit found the three Tauri apps used incompatible styling systems: data-framer
was driven by AG-Grid Quartz `--ag-*` variables, while map-windower and doc-viewer hardcoded
hex and carried a conflicting Inter/`#2563eb` reset. The accent blue was even inconsistent
within a single app (`#2196f3` vs `#2563eb`). This blocked a shared `viewer-ui`.

## Decision
Introduce one neutral, brand-agnostic token set in `@viewers/ui/theme.css`, all prefixed
`--vw-*` so they never collide with third-party themes (`--ag-*`, MapLibre, PDF.js). A fresh
look (indigo accent, slate neutrals, consistent radius/spacing/type scale) rather than
adopting AG-Grid's vocabulary, so the token names aren't coupled to any one library. Light
and dark are both defined; apps follow the OS via `prefers-color-scheme`.

All three apps migrate off hardcoded hex / `--ag-*` **chrome** colors onto `--vw-*`. Where a
third-party library keeps its own theme (AG-Grid's grid, MapLibre's canvas), its variables are
mapped **from** the `--vw-*` tokens rather than duplicated.

## Consequences
- `viewer-ui` components are purely presentational and format-agnostic (props in, events out).
- Visual appearance changes (intentionally — a fresh redesign); requires an in-app visual
  check on each platform, which the headless build/typecheck cannot cover.
- Dark mode is new; third-party surfaces (AG-Grid, MapLibre base map, PDF canvas) may need
  per-app follow-up to look fully native in dark — tracked as polish, not a blocker.
