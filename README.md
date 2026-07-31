# window

A monorepo for four small desktop file-viewer applications that share a framework-free Rust core and (for the Tauri apps) shared frontend packages.

**One repo, one lockfile, one toolchain, four independently-built binaries.**

| App             | Format        | Stack                                  |
| --------------- | ------------- | -------------------------------------- |
| `data-framer`   | CSV / Parquet | Tauri v2 + Vue 3 (Polars)              |
| `map-windower`  | GeoJSON       | Tauri v2 + Vue 3 (MapLibre)            |
| `doc-viewer`    | PDF           | Tauri v2 + Vue 3 (PDF.js)              |
| `image-shutter` | SVG           | egui / eframe + resvg (cargo-packager) |

## Layout

```
crates/    window-core (pure Rust), window-tauri
packages/  @window/ui, @window/bridge, @window/config  (consumed by the three Tauri apps)
apps/      data-framer, map-windower, doc-viewer, image-shutter
```

The egui app (`image-shutter`) is a crate under `apps/`, not `crates/` — it shares
`window-core` but none of the frontend packages.

## Toolchain

- Rust pinned via `rust-toolchain.toml`.
- JS managed with **npm workspaces** (one root `package-lock.json`).
- Task runner: [`just`](https://github.com/casey/just) — run `just` to list recipes.
- Frontend formatting via [Vite+](https://viteplus.dev) (`vp`), a global binary — install with
  `irm https://vite.plus/ps1 | iex` (Windows) or `curl -fsSL https://vite.plus | bash`
  (macOS/Linux). Used by `just fmt`.

## Getting started

```sh
just init                # npm install (JS workspaces)
just dev data-framer     # run a Tauri app in dev
just run image-shutter   # run the egui app
just build               # cargo build --workspace
just fmt                 # cargo fmt + vp fmt
just lint                # clippy -D warnings
```
