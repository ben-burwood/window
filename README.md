# window

A monorepo for four small desktop file-viewer applications that share a framework-free Rust core and (for the Tauri apps) shared frontend packages. 

**One repo, one lockfile, one toolchain, four independently-built binaries.**

| App             | Format        | Stack                                   |
| --------------- | ------------- | --------------------------------------- |
| `data-framer`   | CSV / Parquet | Tauri v2 + Vue 3 (Polars)               |
| `map-windower`  | GeoJSON       | Tauri v2 + Vue 3 (MapLibre)             |
| `doc-viewer`    | PDF           | Tauri v2 + Vue 3 (PDF.js)               |
| `image-shutter` | SVG           | egui / eframe + resvg (cargo-packager)  |

## Layout

```
crates/    viewer-core (pure Rust), viewer-tauri, viewer-egui
packages/  viewer-ui, viewer-bridge, config  (consumed by the three Tauri apps)
apps/      data-framer, map-windower, doc-viewer, image-shutter
```

## Toolchain

- Rust pinned via `rust-toolchain.toml`.
- JS managed with **npm workspaces** (one root `package-lock.json`).
- Task runner: [`just`](https://github.com/casey/just) — run `just` to list recipes.

## Getting started

```sh
just install   # npm install (JS workspaces)
just build     # cargo build --workspace
just check     # cargo metadata resolves
```
