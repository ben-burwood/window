# 0004 — viewer-core is intentionally thin

**Status:** accepted (Phase 3)

## Context

The migration spec imagined a rich `viewer-core` (recents, path canonicalisation, file
sniffing, debounced watch-and-reload, viewport math, PNG export, error types). The Phase 0
audit found that almost none of that exists in Rust today: the three Tauri apps keep their
logic in the Vue/TS frontend, and their backends are thin. The only genuinely-duplicated,
format-agnostic Rust across ≥2 apps is **file sniffing**.

## Decision

`viewer-core` Phase 3 contains exactly what is duplicated and framework-free:

- `sniff::extension_lower` / `sniff::has_extension` — replaces the hand-rolled extension
  checks in all three Tauri apps' `is_supported`.
- `sniff::is_gzip` — the gzip magic-byte sniff, extracted from image-shutter's `maybe_gunzip`.
  Extracting it makes image-shutter a `viewer-core` consumer too, so the crate is genuinely
  shared across **both** the Tauri and egui worlds (the migration's stated thesis).

**Deliberately NOT extracted** (per _"extract on the second occurrence, not the first"_ /
_"no speculative abstraction"_):

- **Path canonicalisation** — one occurrence (image-shutter's `svg.rs`).
- **A shared error enum** — every app is happy with `Result<_, String>`; there is no shared
  error _logic_ to host yet, so an enum would be decorative.
- **File read helpers** — `read_to_string` (map-windower) and `read` (image-shutter) each
  occur once, in incompatible shapes.
- **Viewport pan/zoom math** — exists only in image-shutter (`app.rs`); the Tauri apps do it
  in TS. One Rust occurrence → stays put.
- **Recents, file-watch/reload, PNG export** — do not exist anywhere; they are the deferred
  feature roadmap, to be added when a second consumer actually needs them.

## Consequences

- `viewer-core` stays small and honest; the framework-freedom invariant is trivially held and
  enforced by `just check-core-clean`.
- When any deferred item gains a second real consumer, it is extracted then — not now.
