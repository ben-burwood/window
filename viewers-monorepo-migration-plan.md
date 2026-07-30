# Task: Consolidate four file-viewer apps into a monorepo with a shared core

You are implementing a migration. Four small desktop file-viewer applications currently
live in separate repositories with copy-pasted common components. They need to become one
monorepo: **one repo, one lockfile, one toolchain, four independently-built binaries.**

Work through the phases in order. Each phase ends with a gate you must pass before
continuing. Phase 0 ends with a **hard stop** — report findings and wait for confirmation.

---

## Source repositories

| App             | Format        | Stack                                 | Repo                                           |
| --------------- | ------------- | ------------------------------------- | ---------------------------------------------- |
| `data-framer`   | CSV / Parquet | Tauri v2 + Vue 3                      | `https://github.com/ben-burwood/data-framer`   |
| `map-windower`  | GeoJSON       | Tauri v2 + Vue 3 (MapLibre / deck.gl) | `https://github.com/ben-burwood/map-windower`  |
| `doc-viewer`    | PDF           | Tauri v2 + Vue 3 (PDF.js)             | `https://github.com/ben-burwood/doc-viewer`    |
| `image-shutter` | SVG           | egui / eframe + resvg                 | `https://github.com/ben-burwood/image-shutter` |

---

## Target architecture

Three Rust layers. The key insight is that the Rust split is **not** "Tauri backend vs
egui" — there is a format-agnostic, framework-free layer underneath both shells, and that layer is the main prize of this migration.

- **`viewer-core`** — pure Rust. No `tauri`, no `eframe`, no `egui` in its dependency tree.
  Recents model, path canonicalisation, file sniffing, debounced watch-and-reload,
  pan/zoom math, PNG export, error types.
- **`viewer-tauri`** — `viewer-core` + Tauri plumbing. Single-instance, launch-path
  extraction, window-state persistence, updater, and the `#[tauri::command]` surface.
  Exposes a builder so each app's `main.rs` is nearly empty.
- **`viewer-egui`** — `viewer-core` + eframe plumbing. `rfd` dialogs, hand-rolled
  single-instance, toolbar/side-panel chrome, progressive render loop.

Frontend packages (`viewer-ui`, `viewer-bridge`, `viewer-viewport`, shared configs) are consumed by the three Tauri apps only.

Reference tree (advisory — Phase 0 findings may justify deviation, but flag any deviation explicitly):

```
viewers/
├── Cargo.toml                      # [workspace] members, workspace.dependencies, workspace.lints
├── Cargo.lock
├── rust-toolchain.toml
├── package.json                    # root scripts only, no runtime deps
├── pnpm-workspace.yaml
├── tauri.base.json                 # CSP, updater, bundle publisher/category, window defaults
├── justfile
├── .cargo/config.toml
│
├── crates/
│   ├── viewer-core/src/            # lib.rs, recents.rs, paths.rs, watch.rs,
│   │                               # viewport.rs, export.rs, error.rs
│   ├── viewer-tauri/src/           # lib.rs (app() builder), launch.rs,
│   │                               # single_instance.rs, window_state.rs, commands.rs
│   └── viewer-egui/src/            # lib.rs, single_instance.rs, dialogs.rs,
│                                   # chrome.rs, render_loop.rs
│
├── packages/
│   ├── viewer-ui/src/              # FilePicker, DropZone, EmptyState, ErrorPane,
│   │                               # Toolbar, StatusBar, RecentsList, theme.css
│   ├── viewer-bridge/src/          # typed invoke() wrappers; generated.ts from tauri-specta
│   ├── viewer-viewport/src/        # useViewport.ts
│   └── config/                     # tsconfig/, vite/ (viewerConfig factory), eslint/
│
├── apps/
│   ├── data-framer/                # src/ (Vue) + src-tauri/ (Cargo.toml, tauri.conf.json,
│   ├── map-windower/               #   icons/, capabilities/, src/main.rs + one format module)
│   ├── doc-hopper/
│   └── image-shutter/              # egui: crate IS the app. No src-tauri/, no frontend.
│                                   # Cargo.toml, packager.toml, assets/, src/
│
├── .github/workflows/
│   ├── ci.yml                      # paths-filter → per-app fmt/clippy/test
│   ├── release-tauri.yml           # reusable, matrix over the three Tauri apps
│   └── release-egui.yml            # separate: cargo-packager
│
└── docs/
    ├── adding-a-viewer.md
    └── decisions/
```

---

## Non-negotiable constraints

1. **Four separate binaries.** Never merge into one app. A user opening a 4KB SVG must not download a binary containing Polars.

1. **Never break the build for more than one commit.** Every extraction commit must leave all four apps compiling. Verify, don't assume.
2. **`viewer-core` stays framework-free.** Enforce it — see Phase 4 gate. If something
   needs `tauri`, it belongs in `viewer-tauri`.
3. **Extract on the second occurrence, not the first.** Do not create shared abstractions speculatively. If only one app uses something, it stays in that app.
4. **Shared frontend components take props and know nothing about file formats.** The moment `FilePicker.vue` needs to know what a PDF is, that logic belongs in the app. This is the primary failure mode of this refactor; guard against it actively.

---

## Explicit non-goals

Do not do these, even though they may look like natural next steps:

- **No `trait Viewer` / `trait Renderer` abstraction.** A table grid, deck.gl, PDF.js and
  an SVG DOM share nothing. Such a trait would be decorative and would invite format awareness into the shared layer.
- **No runtime plugin system.** No dylib loading, no WASM component model. Compile-time modularity via the workspace is the whole goal.
- **No unifying the two viewport implementations.** `viewer-core::viewport` (Rust, for egui) and `viewer-viewport` (TS, for the webviews) are intentional twins of ~40 lines each. Pan events at 60fps must not cross the IPC bridge. Record this in
  `docs/decisions/`.
- **No merging `capabilities/`.** Tauri v2 permissions stay per-app; merging grants the union of filesystem scopes to every app.
- **No bending shared crates to accommodate `image-shutter`.** It shares `viewer-core` and nothing else. If you find yourself adding an abstraction so the egui app can reuse a Vue component, stop.
- **No feature-flag monolith** (`--features pdf|svg|csv|geo` from one crate). Considered and rejected: `#[cfg]` sprawl for no gain over the workspace.

---

## Phase 0 — Audit, then STOP

Clone all four repos read-only and report. Do not create the monorepo yet.

Produce a written report covering:

1. **Version drift** — per app: Tauri version, Rust edition/toolchain, Vue version, Vite
   version, package manager and lockfile type, Node version.
2. **The actual duplication** — diff the copy-pasted components across repos. For each
   candidate shared item (file picker, drop zone, empty state, toolbar, recents, pan/zoom, launch-path handling, window state), state: which apps have it, whether the copies have diverged, and if so how.
3. **Launch-path handling** — how each Tauri app currently receives a file path on
   double-click (`argv`, `RunEvent::Opened`, plugin, or not implemented). Note which apps have single-instance handling.
4. **Existing `tauri.conf.json` diff** — which keys are genuinely per-app vs identical
   across all three.
5. **Rust code inventory per app** — what's in `src-tauri/src/`. Flag anything
   format-agnostic (candidate for `viewer-core`) vs format-specific (stays).
6. **Frontend styling approach** — plain CSS, Tailwind, CSS modules, something else. If
   they differ, this blocks `viewer-ui` and needs a decision.
7. **`image-shutter` status** — exists, partially exists, or needs scaffolding.
8. **Anything that contradicts the target architecture above.**

**Then stop and wait.** Do not proceed to Phase 1 until the report is reviewed and
confirmed. The architecture in this document was designed without sight of the code; your audit is the first contact with reality and may change it.

---

## Phase 1 — Skeleton

In the fresh repo:

- Root `Cargo.toml` with `[workspace]`, `[workspace.package]` (version, edition,
  rust-version, license), `[workspace.dependencies]`, `[workspace.lints.clippy]`,
  `[profile.release]` (lto, strip, codegen-units).
- `rust-toolchain.toml` pinning one toolchain.
- `pnpm-workspace.yaml` covering `apps/*` and `packages/*`.
- Root `package.json` with scripts only — no runtime dependencies.
- `.cargo/config.toml`, `.gitignore`, `justfile` stub, `README.md` stub.
- Empty `crates/`, `packages/`, `apps/`, `docs/decisions/`.
- `justfile` for build commands, formatting commands etc.

**Gate:** `cargo metadata` succeeds. `pnpm install` succeeds. Commit.

---

## Phase 2 — Workspace unification

Copy all code from the 4 indepdent repos.

- Add every Rust crate to workspace `members`.
- Hoist shared deps to `[workspace.dependencies]`; apps use
  `tauri = { workspace = true }`. Resolve version drift to the newest common version, one app at a time.
- Delete per-app `Cargo.lock` files; regenerate one at root.
- Delete per-app lockfiles for the frontend; regenerate one `pnpm-lock.yaml`.
- Reconcile Node/pnpm versions.

**Gate:** all four apps build and launch from the monorepo. `cargo build --workspace`
succeeds. Confirm the shared `target/` is being used (dependencies compile once, not four times). 

---

## Phase 3 — Extract `viewer-core`

The first real extraction. Move only what the Phase 0 audit identified as
format-agnostic and used by two or more apps.

Likely contents: recents model, path canonicalisation, file sniffing, `notify` wrapper with debounce, viewport math, error types, `resvg`-based PNG export.

**Gate — verify framework-freedom mechanically:**

```
cargo tree -p viewer-core | grep -Ei 'tauri|eframe|egui|wry'   # must be empty
```

All four apps build. All four still launch and open a file. 

---

## Phase 4 — Extract `viewer-tauri`

Move single-instance, launch-path extraction, window-state persistence, updater config, and the command surface out of the three Tauri apps.

Design the public API as a builder such that each app's `main.rs` reduces to roughly:

```rust
fn main() {
    viewer_tauri::app()
        .associations(&["pdf"])
        .invoke_handler(tauri::generate_handler![doc_hopper::outline])
        .run();
}
```

Handle the platform asymmetry inside the crate: Windows and Linux receive the path via `argv`; macOS receives an Apple Event surfacing as `RunEvent::Opened`.

**Gate:** each `apps/*/src-tauri/src/main.rs` is under ~20 lines and contains only builder calls. Each app has at most one format-specific Rust module beside it. **Manually verify file-association double-click on every platform you can reach** — this is the fiddliest code in the repo and the one thing unit tests won't catch.

---

## Phase 5 — Extract frontend packages

- `viewer-ui` — the format-agnostic Vue components. Props in, events out, zero format knowledge.
- `viewer-bridge` — the **only** place `invoke("...")` string literals appear. Investigate `tauri-specta` to generate types from the Rust command signatures instead of hand-writing them; if you adopt it, decide and document whether `generated.ts` is committed or gitignored.
- `viewer-viewport` — `useViewport.ts`. If it turns out to be trivial enough that it never gets used without `viewer-ui`, fold it in and note the decision.
- `packages/config/` — shared tsconfig base, eslint config, and a **`viewerConfig(opts)`
  Vite factory** (a factory, not a static config file: each app needs its own paths and
  lazy chunks).

**Gate:** no duplicated component files remain across the three Tauri apps. `grep -r
'invoke(' apps/` returns nothing outside `viewer-bridge`. All three apps build and run.

---

## Phase 6 — Config consolidation

Tauri v2 merges configs: `tauri build --config <path>` layers a file over the app's own,
with objects merging and arrays replacing.

- Root `tauri.base.json`: CSP, updater endpoints, bundle publisher/category/copyright,
  window defaults.
- Each `apps/*/src-tauri/tauri.conf.json`: only `productName`, `identifier`,
  `fileAssociations`, icons, and genuine per-app overrides.
- Wire the merge into `justfile` recipes so nobody can build without it.

**Important:** verify the current merge semantics against the Tauri v2 docs for the pinned
version before relying on this. The behaviour has moved between versions. If merge is
unreliable, fall back to generating each `tauri.conf.json` from the base via a build script
and report the change.

`image-shutter` gets `packager.toml` for `cargo-packager` (file associations, bundling). It
does not participate in `tauri.base.json`.

**Gate:** each app's `tauri.conf.json` is under ~30 lines. A merged build produces a bundle
with correct associations, publisher metadata, and CSP. Commit.

---

## Phase 7 — CI

- `ci.yml` — `dorny/paths-filter` so touching the SVG app doesn't rebuild the other three.
  Per-app `fmt`, `clippy`, `test`, typecheck.
- `release-tauri.yml` — reusable workflow, matrix over `[data-framer, map-windower,
  doc-hopper]` × platforms.
- `release-egui.yml` — separate workflow. `image-shutter` bundles via `cargo-packager`, not
  `tauri-cli`; its steps differ enough that it is not a matrix row.
- **One cargo cache keyed on the root `Cargo.lock`.** This is where the shared `target/`
  pays off most; get the cache key right.
- Signing and updater secrets configured once.

**Gate:** a tag produces artifacts for all four apps. A commit touching only
`apps/image-shutter/` triggers only its jobs.

---

## Conventions

- Rust: `cargo fmt`, `clippy -D warnings`. Idiomatic and minimal; no dependency added without justification in the commit message.
- Prefer stdlib and existing workspace deps over new crates.

- Match the existing code style found in Phase 0 rather than imposing a new one.

## Stop and ask rather than guessing

- The three Tauri apps use different CSS/styling approaches (blocks `viewer-ui`).
- Version drift can't be resolved to a common version without a breaking upgrade.
- Tauri config merge doesn't behave as described for the pinned version.
- An extraction would require format-specific logic in a shared crate or component.
- 
- Any phase gate fails twice.
