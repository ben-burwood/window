//! `viewer-tauri` — the Tauri plumbing shared by the three Tauri viewer apps.
//!
//! Extracted from the copy-pasted `AppState.startup_file` + argv `.setup()` closure +
//! `get_startup_file` command that appeared identically in data-framer, map-windower and
//! doc-viewer. Exposed as a builder so each app's `run()` is just builder calls plus its own
//! state and format-specific commands.
//!
//! ```ignore
//! // (illustrative — `generate_context!` needs the app's own tauri.conf.json to compile)
//! #[cfg_attr(mobile, tauri::mobile_entry_point)]
//! pub fn run() {
//!     viewer_tauri::app(&["pdf"])
//!         .invoke_handler(tauri::generate_handler![viewer_tauri::get_startup_file, app::outline])
//!         .run(tauri::generate_context!())
//!         .expect("error while running tauri application");
//! }
//! ```
//!
//! `generate_handler!` and `generate_context!` must be invoked in the app crate (they read
//! the app's own commands and `tauri.conf.json`), so they can't be hidden inside `app()`;
//! each app therefore lists `viewer_tauri::get_startup_file` in its own handler.
//!
//! Deferred (see `docs/decisions/`): single-instance, macOS `RunEvent::Opened` (double-click
//! file-open), window-state persistence, and the updater. `app()` is shaped so these can be
//! added here later without touching the apps.

use tauri::{Builder, Manager, Wry};

// `#[tauri::command]` emits a `#[macro_export]` helper macro that lands in the crate root's
// macro namespace; defining the command in a submodule (rather than directly in lib.rs) keeps
// that from colliding with the macro's own re-export. The command is re-exported below so
// apps still reference it as `viewer_tauri::get_startup_file`.
mod command {
    use std::sync::Mutex;
    use tauri::State;

    /// Holds the path the app was launched with (OS file association / double-click), so the
    /// frontend can pick it up once on startup.
    #[derive(Default)]
    pub struct StartupFile(pub(crate) Mutex<Option<String>>);

    /// Returns and clears the startup file path. The frontend calls this once on mount;
    /// subsequent calls return `None`.
    #[tauri::command]
    pub fn get_startup_file(state: State<'_, StartupFile>) -> Option<String> {
        state.0.lock().unwrap().take()
    }
}

pub use command::{get_startup_file, StartupFile};

/// Begin building a viewer app: registers the common plugins (`opener`, `dialog`), manages
/// [`StartupFile`] state, and installs an argv launch-path handler that stores `argv[1]` when
/// its extension is one of `extensions` (given without the leading dot, e.g. `&["pdf"]`).
///
/// The caller adds any app-specific `.manage(...)`, then finishes with
/// `.invoke_handler(tauri::generate_handler![viewer_tauri::get_startup_file, ..])` and
/// `.run(tauri::generate_context!())`.
pub fn app(extensions: &[&str]) -> Builder<Wry> {
    let exts: Vec<String> = extensions.iter().map(|s| s.to_lowercase()).collect();
    Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(StartupFile::default())
        .setup(move |app| {
            let args: Vec<String> = std::env::args().collect();
            if let Some(path) = args.get(1) {
                let ext_refs: Vec<&str> = exts.iter().map(String::as_str).collect();
                if viewer_core::has_extension(path, &ext_refs) {
                    *app.state::<StartupFile>().0.lock().unwrap() = Some(path.clone());
                }
            }
            Ok(())
        })
}
