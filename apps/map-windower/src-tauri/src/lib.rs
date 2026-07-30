mod geoparquet;

// Reads a GeoJSON file from disk and returns its raw text. The frontend parses
// and validates it.
#[tauri::command]
fn load_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {path}: {e}"))
}

// Reads a GeoParquet file and returns it as a GeoJSON FeatureCollection string,
// which the frontend renders exactly like a plain GeoJSON file.
#[tauri::command]
fn load_geoparquet(path: String) -> Result<String, String> {
    geoparquet::to_geojson(&path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    viewer_tauri::app(&["geojson", "pmtiles", "geoparquet"])
        .invoke_handler(tauri::generate_handler![
            viewer_tauri::get_startup_file,
            load_file,
            load_geoparquet
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
