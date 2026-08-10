# Adding a new viewer

This monorepo is built so a new file-viewer is mostly wiring, not boilerplate. There are two
kinds of viewer; pick the one that matches your rendering stack.

## A new Tauri + Vue viewer (like data-framer / map-windower / doc-viewer)

1. **Scaffold** `apps/<name>/` with `src/` (Vue) and `src-tauri/` (Cargo + `tauri.conf.json` +
   `capabilities/` + `icons/`). Copy the shape of an existing app — doc-viewer is the smallest.
2. **Rust** (`apps/<name>/src-tauri/`):
   - `Cargo.toml`: use the workspace fields (`version.workspace = true`, etc.), depend on
     `window-core`, `window-tauri`, `tauri`, and the two plugin crates via `{ workspace = true }`
     (the plugins must stay _direct_ deps so their ACL permissions are discovered), plus your
     format-specific crates.
   - `src/main.rs`: `fn main() { <name>_lib::run() }`.
   - `src/lib.rs`: `window_tauri::app(&["ext"]) .invoke_handler(tauri::generate_handler![
window_tauri::get_startup_file, /* your commands */ ]).run(tauri::generate_context!())`.
     Keep format logic in one module beside `lib.rs`.
   - Add the crate to the root `Cargo.toml` `[workspace] members`.
3. **Frontend** (`apps/<name>/`):
   - `package.json`: depend on `@window/ui`, `@window/bridge` (and `@window/config` in dev),
     named `"*"`.
   - `vite.config.ts`: `export default viewerConfig({ manualChunks: { … } })`.
   - `tsconfig.json`: `"extends": "@window/config/tsconfig.base.json"`.
   - `src/main.ts`: `import "@window/ui/theme.css"` first.
   - `src/bridge.ts`: this app's typed `invoke` wrappers (built on `@window/bridge`). Never
     call `invoke("…")` in a component — go through the bridge.
   - Use `<Toolbar>`, `<ToolbarButton>`, `<EmptyState>` from `@window/ui`; style with the
     `--vw-*` tokens, never hardcoded hex.
4. **Config**: keep `tauri.conf.json` minimal (productName, identifier, version,
   `build.frontendDist`, `app.windows`, `bundle.icon`, `bundle.fileAssociations`, and
   `app.security.assetProtocol` if you need it). Shared keys come from `tauri.base.json`.
5. **CI/Release**: add the app to the `paths-filter` block and matrices in
   `.github/workflows/ci.yml` and `release-tauri.yml`.
6. **Verify**: `npm run build --workspace <name>` (frontend), `cargo build -p <name>`, a bundle
   via `npm run tauri --workspace <name> -- build --config ../../tauri.base.json` (or the
   release workflow), and a manual launch + double-click-open smoke test.

## A new egui viewer (like image-shutter / drawing-paner)

The crate _is_ the app — no `src-tauri/`, no frontend. Depend on `window-core` for the shared
framework-free helpers (sniffing, watching, etc.), keep your `[package.metadata.packager]` config
for `cargo-packager`, and add the crate to `[workspace] members`. Do **not** pull it into the Tauri
shared frontend packages.

- **CI** (`.github/workflows/ci.yml`): add a `paths-filter` entry and a matrix row with
  `"deps":"egui","web":""` (see the `image-shutter` / `drawing-paner` lines).
- **Release** (`.github/workflows/release-egui.yml`): add the crate name to the `app:` matrix list.
  The workflow builds every app in that list across all three platforms, so no other change is
  needed there.

## The rules that keep this working

- Shared crates/packages know **nothing** about file formats. Format knowledge stays in the app.
- Extract into a shared layer only on the **second** occurrence, never speculatively.
- `window-core` must stay framework-free — `cargo tree -p window-core` must show no
  tauri/eframe/egui/wry.
