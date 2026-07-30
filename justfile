# Task runner for the viewers monorepo. Recipes are fleshed out per phase.
# Run `just` to list available recipes.

default:
    @just --list

# --- Rust ---------------------------------------------------------------------

# Build every crate in the workspace (shared target/).
build:
    cargo build --workspace

# Format all Rust code.
fmt:
    cargo fmt --all

# Check formatting without writing (CI).
fmt-check:
    cargo fmt --all -- --check

# Clippy across the workspace, warnings are errors (CI).
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Run all Rust tests.
test:
    cargo test --workspace

# Enforce that viewer-core stays framework-free (Phase 3 gate).
# Fails if tauri/eframe/egui/wry appear in its dependency tree.
check-core-clean:
    cargo tree -p viewer-core -e no-dev | grep -Eiq 'tauri|eframe|egui|wry' && (echo "viewer-core is not framework-free" && exit 1) || echo "viewer-core is framework-free"

# --- Frontend -----------------------------------------------------------------

# Install all JS workspace dependencies.
install:
    npm install

# --- Meta ---------------------------------------------------------------------

# Verify the workspace resolves (Phase 1 gate).
check:
    cargo metadata --format-version 1 > /dev/null && echo "cargo metadata OK"
