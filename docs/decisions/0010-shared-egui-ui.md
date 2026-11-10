# 0010 — Shared egui chrome (`window-egui`)

**Status:** accepted

## Context

The Tauri apps share one look through `@window/ui` (`--vw-*` tokens, `Toolbar`, `ToolbarButton`,
`Badge`, `EmptyState`). The five egui apps used egui's default visuals and each hand-rolled the
same chrome: a `columns(3)` toolbar with a row-height hack, an "outdated" pill with hardcoded
reds, emoji buttons, `LIGHT_RED` error text, a bare "Open a … to get started" label, and the
window-title bookkeeping. Five copies is well past ADR 0004's "extract on the second occurrence".

## Decision

Add `crates/window-egui` (depends on `egui` only):

- `tokens` — the `--vw-*` colours, radii, spacing and type scale, hand-mirrored from
  `packages/ui/src/theme.css`. A unit test parses `theme.css` and fails if the light or dark
  colours drift from the Rust copy.
- `install(ctx)` — registers a light and a dark `Style` built from the tokens, so egui keeps
  following the OS theme like the Tauri apps' `prefers-color-scheme` default.
- `widgets` — `Toolbar` (start/center/end slots), `toggle_button`, `primary_button`, `badge` /
  `outdated_badge`, `empty_state`, `file_label`, `error_text`, `set_title`.

The egui default font is kept (no bundled Inter), and toolbar buttons are text-only.

## Consequences

- egui apps match the Tauri apps' palette, toolbar, badges and empty state, in light and dark.
- `theme.css` stays the source of truth; changing a colour there means updating `tokens.rs`,
  which CI enforces.
- Like the Tauri apps, viewers hide the toolbar until a file is loaded and show the
  `empty_state` card (with any load error) instead.
