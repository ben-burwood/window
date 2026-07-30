//! `window-tauri` — the Tauri plumbing shared by the three Tauri viewer apps.
//!
//! Extracted from the copy-pasted launch-path handling that appeared identically in every app,
//! and now the home of file-open routing:
//! - **Windows/Linux:** each file-open launches its own process (`argv[1]`, handled in
//!   [`app`]'s setup). This is intentionally *not* single-instance — opening two files gives
//!   two independent windows.
//! - **macOS:** the OS runs one process per app and delivers every file-open (including the
//!   first) as an Apple Event surfacing as `RunEvent::Opened`, handled in [`run`]. That variant
//!   only exists on macOS, so the whole routing path is `#[cfg(target_os = "macos")]`.
//!
//! Apps build with [`app`] and finish with [`run`] (which owns the event loop):
//! ```ignore
//! #[cfg_attr(mobile, tauri::mobile_entry_point)]
//! pub fn run() {
//!     window_tauri::run(
//!         window_tauri::app(&["pdf"])
//!             .invoke_handler(tauri::generate_handler![window_tauri::get_startup_file]),
//!         tauri::generate_context!(),
//!     );
//! }
//! ```
//! The initial file is pulled once via [`get_startup_file`]; macOS runtime opens are pushed to
//! the frontend via the `open-file` event (see `@window/bridge`'s `onOpenFile`).

use tauri::{Builder, Context, Manager, Wry};

// `#[tauri::command]` emits a `#[macro_export]` helper macro that lands in the crate root's
// macro namespace; defining the command in a submodule keeps it from colliding with the
// macro's own re-export. Re-exported below so apps reference `window_tauri::get_startup_file`.
mod command {
    use std::sync::Mutex;
    use std::time::Duration;
    use tauri::{AppHandle, Emitter, State};

    /// Emitted (debounced) when the open file changes on disk. The frontend flags the file as
    /// outdated — it does not reload (Tier 1.3 is the detect half only).
    pub const FILE_CHANGED_EVENT: &str = "file-changed";

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

    /// Holds the active file watcher; replaced whenever a new file is watched, dropped on exit.
    #[derive(Default)]
    pub struct WatchState(Mutex<Option<window_core::FileWatch>>);

    /// Watch `path` and emit [`FILE_CHANGED_EVENT`] (debounced) whenever it changes on disk.
    /// Replaces any previous watch. Call this after loading a file; the frontend then shows an
    /// "outdated" badge until the user reopens the file.
    #[tauri::command]
    pub fn watch_file(
        path: String,
        app: AppHandle,
        state: State<'_, WatchState>,
    ) -> Result<(), String> {
        let handle = app.clone();
        let watch = window_core::watch_file(&path, Duration::from_millis(300), move || {
            let _ = handle.emit(FILE_CHANGED_EVENT, ());
        })?;
        *state.0.lock().unwrap() = Some(watch);
        Ok(())
    }
}

pub use command::{get_startup_file, watch_file, StartupFile, WatchState};

/// Begin building a viewer app: registers the common plugins (`opener`, `dialog`), manages
/// startup-file state, and installs the first-launch argv handler. `extensions` are the
/// accepted file extensions (without the leading dot, e.g. `&["pdf"]`).
///
/// The caller adds any app-specific `.manage(...)` and `.invoke_handler(...)`, then passes the
/// builder to [`run`].
pub fn app(extensions: &[&str]) -> Builder<Wry> {
    let exts: Vec<String> = extensions.iter().map(|s| s.to_lowercase()).collect();

    // macOS routes runtime file-opens through `RunEvent::Opened`, which needs the accepted
    // extensions available in the event loop, so it stashes them in managed state.
    #[cfg(target_os = "macos")]
    let macos_exts = exts.clone();

    let builder = Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(StartupFile::default())
        .manage(WatchState::default())
        .setup(move |app| {
            // First launch on Windows/Linux delivers the path via argv (macOS uses the Apple
            // Event handled in `run`). `args_os` avoids a panic on non-UTF-8 argv.
            if let Some(path) = std::env::args_os()
                .nth(1)
                .and_then(|s| s.into_string().ok())
            {
                let refs: Vec<&str> = exts.iter().map(String::as_str).collect();
                if window_core::has_extension(&path, &refs) {
                    *app.state::<StartupFile>().0.lock().unwrap() = Some(path);
                }
            }
            Ok(())
        });

    #[cfg(target_os = "macos")]
    let builder = builder.manage(macos::Extensions(macos_exts));

    builder
}

/// Build and run the app, owning the event loop so macOS file-open Apple Events
/// (`RunEvent::Opened`) route to the frontend. Apps call this instead of `.run(context)`.
pub fn run(builder: Builder<Wry>, context: Context) {
    let app = builder
        .build(context)
        .expect("error while building tauri application");
    app.run(move |_app_handle, _event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = _event {
            for url in urls {
                if let Ok(path) = url.to_file_path() {
                    if let Some(path) = path.to_str() {
                        macos::route_open(_app_handle, path);
                    }
                }
            }
        }
    });
}

/// macOS-only file-open routing. `RunEvent::Opened` is the *only* way files reach a macOS app
/// (one process per bundle), so on other platforms none of this is compiled.
#[cfg(target_os = "macos")]
mod macos {
    use super::StartupFile;
    use tauri::{AppHandle, Emitter, Manager};

    /// Event pushed to the frontend when a file should open at runtime.
    const OPEN_FILE_EVENT: &str = "open-file";

    /// Accepted launch extensions, kept in managed state so the Apple-Event handler can
    /// validate incoming paths (the frontend never receives an unsupported file).
    pub struct Extensions(pub Vec<String>);

    fn supported(app: &AppHandle, path: &str) -> bool {
        let exts = app.state::<Extensions>();
        let refs: Vec<&str> = exts.0.iter().map(String::as_str).collect();
        window_core::has_extension(path, &refs)
    }

    /// Route a file to the running app: stash it (so a not-yet-mounted webview can't miss it —
    /// the frontend pulls it via `get_startup_file` and dedupes by path) and, if the window is
    /// up, focus it and push the open-file event.
    pub fn route_open(app: &AppHandle, path: &str) {
        if !supported(app, path) {
            return;
        }
        *app.state::<StartupFile>().0.lock().unwrap() = Some(path.to_string());
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
            let _ = app.emit(OPEN_FILE_EVENT, path.to_string());
        }
    }
}
