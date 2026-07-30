# 0008 — CI and release workflows

**Status:** accepted (Phase 7)

## Decisions
- **`ci.yml` uses a `dorny/paths-filter` dynamic matrix.** A `changes` job maps changed paths
  to a JSON matrix of affected crates; a change under `apps/image-shutter/` runs only
  image-shutter's job, while a change to shared code (root manifests, `crates/`, `packages/`,
  `tauri.base.json`, workflows) runs everything. `fmt` is a single fast whole-tree job.
- **Tauri app crates build their frontend before `cargo clippy`/`test`.** `generate_context!`
  embeds `../dist` at compile time, so the job runs `npm ci` + `npm run build --workspace
  <app>` first — which also performs the `vue-tsc` typecheck (so no separate typecheck job).
- **One shared cargo cache** via `Swatinem/rust-cache` keyed on the single root `Cargo.lock`
  (`shared-key`), so the shared `target/` pays off across app jobs.
- **`release-tauri.yml`** is reusable (`workflow_call`) + tag-triggered, a matrix over the
  three Tauri apps × {macOS universal, Ubuntu, Windows} via `tauri-apps/tauri-action`, passing
  `--config <abs>/tauri.base.json` so the shared bundle metadata is always merged.
- **`release-egui.yml`** is separate (image-shutter bundles with `cargo-packager`, not
  tauri-cli), adapted from image-shutter's original workflow, repointed to `apps/image-shutter`
  and keyed off the git tag (the crate no longer carries a literal version — it inherits the
  workspace version).
- **Signing/updater secrets are wired once** in `release-tauri.yml` env (Apple + Tauri updater
  keys). The updater itself is a deferred feature, so the signing secrets may be unset until
  it's enabled; the build still succeeds unsigned.
- Per-app `.github/` folders copied from the source repos were deleted (GitHub only reads the
  root `.github/`, so they were inactive clutter).

## Consequences
- Selection logic verified by local simulation (image-shutter-only → only its job; shared →
  all). The remaining gate — "a tag produces artifacts for all four apps" — can only be
  exercised by pushing a tag on GitHub; the workflow YAML is syntactically valid but unrun.
- Pinned toolchain: jobs run `rustup show`, which installs the `rust-toolchain.toml` toolchain
  (1.94.0) + clippy/rustfmt automatically.
