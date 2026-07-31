# 0009 — File-open handling (multi-window, macOS Apple Events)

**Status:** accepted

## Context

Opening a file needs to work from the file dialog, the initial launch (OS file association /
double-click), OS drag-and-drop, and — on macOS — the OS delivering opens to an
already-running app. Before this, only the file dialog and a single argv-on-launch path
existed; macOS file-open didn't work at all, and only doc-viewer had drag-and-drop.

## Decisions

- **No single-instance (intentional multi-window).** Opening two files opens two windows. On
  Windows/Linux this is the native behavior — each file-open is its own process — so we simply
  do **not** add `tauri-plugin-single-instance`. Each process handles its own `argv[1]`.
- **macOS file-open via `RunEvent::Opened`.** macOS runs one process per app bundle and
  delivers every file-open (including the first) as an Apple Event. `window-tauri::run` owns the
  event loop and handles `RunEvent::Opened`, routing the path to the frontend. This whole path
  is `#[cfg(target_os = "macos")]` — the variant doesn't exist on other targets, and gating it
  keeps Windows/Linux free of dead code (so `clippy -D warnings` stays clean).
- **`window_tauri::run(builder, context)` wrapper.** Apps call this instead of `.run()` so the
  event loop lives in the shared crate — future window/event handling is added here without
  touching the apps (this was review finding F3).
- **Two delivery channels to the frontend.** The initial file is stashed and pulled once via
  `get_startup_file`; macOS runtime opens are pushed via an `open-file` event
  (`@window/bridge` `onOpenFile`). Every source — dialog, startup, drag-drop, open-file —
  funnels through a single per-app `openPath()` that dedupes by path, so the same file never
  loads twice.
- **Drag-and-drop for all three Tauri apps** via `@window/bridge` `onFileDrop`; each app
  validates the dropped file against its own accepted extensions (format knowledge stays in
  the app) and shows a clear error otherwise.
- **UTF-8-safe argv.** Launch-path parsing uses `args_os().nth(1)` (not `args()`, which panics
  on non-UTF-8 arguments — review finding C1).

## Consequences

- Windows/Linux get true multi-window for free; macOS opens route through one process (a future
  enhancement could spawn a window per `Opened` for multi-window parity on macOS).
- The macOS routing is untestable in the headless/Windows dev environment — verify with
  `open -a <App> file` and Finder double-click on a Mac.
