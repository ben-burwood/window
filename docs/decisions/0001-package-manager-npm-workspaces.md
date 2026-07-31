# 0001 — Package manager: npm workspaces (not pnpm)

**Status:** accepted (Phase 1)

## Context

The original migration spec assumed pnpm (`pnpm-workspace.yaml`, `pnpm-lock.yaml`). The Phase 0
audit found all three Tauri apps actually use **npm** with a `package-lock.json`; there is no
pnpm anywhere and no Node/pnpm version pin.

## Decision

Use **npm workspaces**. Root `package.json` declares `"workspaces": ["apps/*", "packages/*"]`
and there is a single root `package-lock.json`. No `pnpm-workspace.yaml`.

## Consequences

- Zero migration churn for existing app tooling; `@tauri-apps/cli` and Vite configs work
  unchanged.
- Dependencies hoist to the root `node_modules`; per-app `package-lock.json` files were
  deleted and regenerated as one root lock.
- If pnpm's stricter resolution or content-addressed store is wanted later, revisit — the
  workspace layout is compatible with a future pnpm migration.
