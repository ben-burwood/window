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
