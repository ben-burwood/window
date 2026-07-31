# 0006 — Frontend package boundaries (bridge, ui, config; no viewer-viewport)

**Status:** accepted (Phase 5)

## Context

Phase 5 extracts shared frontend packages. Two spec points needed interpreting against the
audit reality.

## Decisions

- **`@window/bridge` holds shared invoke literals + a generic `invoke`.** The audit found
  `load_file` means different things in data-framer vs map-windower, so app-specific commands
  must **not** live in the shared bridge (that would make it format-aware — the migration's
  primary failure mode). Instead: shared commands (`get_startup_file`) and dialog/asset
  helpers live in `@window/bridge`; each app keeps a single local `src/bridge.ts` for its own
  typed command wrappers, built on the re-exported `invoke`. Components never call `invoke`
  with a raw string — the literals are confined to bridge modules. (This is the spirit of the
  spec's "invoke only in viewer-bridge" gate; a strict single-file reading is impossible
  without format knowledge leaking into the shared package.)
- **`tauri-specta` not adopted (yet).** It would add a Rust dependency + a codegen step + a
  commit/gitignore decision to every app for a handful of commands. Hand-written typed
  wrappers in `bridge.ts` are simpler now; revisit if the command surface grows.
- **No `viewer-viewport` package.** No `useViewport` composable exists: map pan/zoom is
  MapLibre-native and doc-viewer's zoom is locked inside `PdfView.vue`. Creating a shared
  composable now would be speculative; deferred until a real second consumer appears.
- **`@window/config` is a Vite _factory_ + tsconfig base + (opt-in) eslint config**, matching
  the spec: each app calls `viewerConfig({ manualChunks })` for its own lazy chunks.

## Consequences

- The shared frontend packages carry zero format knowledge.
- `grep "invoke(" apps/` is confined to each app's `bridge.ts`; components are clean.
