set shell := ["powershell.exe", "-c"]

default:
    @just --list

init:
    npm install

dev app:
    npm run tauri --workspace {{app}} -- dev --config ../../tauri.base.json

run app *args:
    cargo run -p {{app}} -- {{args}}

build:
    cargo build --workspace

test:
    cargo test --workspace

fmt:
    cargo fmt --all
    vp fmt --write

lint:
    cargo clippy --workspace --all-targets -- -D warnings

# First-time WinGet submission for new packages (e.g. `just winget-new 0.1.3 doc-viewer map-windower`).
# Add -DryRun by omitting apps and editing, or run the WinGet New workflow in CI. Needs $env:WINGET_TOKEN.
winget-new version *apps:
    ./scripts/winget-new.ps1 -Version {{version}} -Apps ({{apps}} -split ' ')

# Bump all npm and cargo deps to latest (including major versions).
bump-dependencies: bump-npm bump-cargo

# Bump frontend deps across every workspace (apps/* and packages/*) to latest, then refresh the lockfile.
bump-npm:
    npx npm-check-updates -u --workspaces --root
    npm install

# Bump Rust deps across the workspace (incl. [workspace.dependencies]) to latest, then update transitive deps.
# Needs cargo-edit (`cargo install cargo-edit`). Ignores the workspace `rust-version`, which otherwise caps
# "latest" at MSRV-compatible releases and would downgrade crates like egui and arrow.
# `cargo upgrade` re-resolves the lockfile offline, so refresh the index first or it can pick stale/yanked crates.
bump-cargo:
    cargo update
    cargo upgrade --incompatible --ignore-rust-version
    cargo update
