// doc-viewer has no Rust-side format logic: PDF rendering is entirely in the frontend
// (PDF.js). All this backend does is the shared launch-path plumbing, provided by
// window-tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    window_tauri::run(
        window_tauri::app(&["pdf"])
            .invoke_handler(tauri::generate_handler![window_tauri::get_startup_file]),
        tauri::generate_context!(),
    );
}
