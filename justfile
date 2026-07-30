set shell := ["powershell.exe", "-c"]

default:
    @just --list

init:
    npm install

dev app:
    npm run tauri --workspace {{app}} -- dev --config ../../tauri.base.json

# Run a non-Tauri (egui) app, e.g. `just run image-shutter [path/to/file]`.
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
