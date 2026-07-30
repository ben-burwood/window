// doc-viewer has no Rust-side format logic: PDF rendering is entirely in the frontend
// (PDF.js). All this backend does is the shared launch-path plumbing, provided by
// viewer-tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    viewer_tauri::app(&["pdf"])
        .invoke_handler(tauri::generate_handler![viewer_tauri::get_startup_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
