use std::sync::Mutex;
use tauri::{Manager, State};

// Holds the file path the app was launched with (via OS file association /
// double-click), so the frontend can pick it up once on startup.
#[derive(Default)]
struct AppState {
    startup_file: Mutex<Option<String>>,
}

fn is_supported(path: &str) -> bool {
    viewer_core::has_extension(path, &["pdf"])
}

// Returns and clears the startup file path. The frontend calls this once on
// mount; subsequent calls return None.
#[tauri::command]
fn get_startup_file(state: State<'_, AppState>) -> Option<String> {
    state.startup_file.lock().unwrap().take()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            if let Some(path) = args.get(1) {
                if is_supported(path) {
                    *app.state::<AppState>().startup_file.lock().unwrap() = Some(path.clone());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_startup_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
