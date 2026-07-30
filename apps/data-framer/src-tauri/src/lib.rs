mod datastore;

use datastore::{AppState, FileInfo, LoadedFile, RowsResponse};
use std::sync::Mutex;
use tauri::{Manager, State};

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Load a parquet or CSV file: reads schema + row count (no data), stores
/// the file path in managed state, and returns metadata to the frontend.
#[tauri::command]
fn load_file(path: String, state: State<'_, AppState>) -> Result<FileInfo, String> {
    let mut lf = datastore::scan_file(&path)?;
    let columns = datastore::extract_schema(&mut lf)?;
    let total_rows = datastore::count_rows(lf, &path)?;

    *state.file.lock().unwrap() = Some(LoadedFile {
        path: path.clone(),
        total_rows,
        schema: columns.clone(),
    });

    Ok(FileInfo {
        path,
        total_rows,
        columns,
    })
}

/// Return a paginated, optionally sorted, optionally filtered, optionally column-projected
/// slice of the loaded file. Only `limit` rows are collected and sent over IPC.
#[tauri::command]
fn get_rows(
    offset: i64,
    limit: i64,
    sort_col: Option<String>,
    sort_desc: bool,
    filters: Vec<datastore::FilterSpec>,
    columns: Vec<String>,
    state: State<'_, AppState>,
) -> Result<RowsResponse, String> {
    let (file_path, unfiltered_rows, schema) = {
        let guard = state.file.lock().unwrap();
        let loaded = guard.as_ref().ok_or("No file loaded")?;
        (loaded.path.clone(), loaded.total_rows, loaded.schema.clone())
    };

    let lf = datastore::build_pipeline(
        &file_path,
        &filters,
        &schema,
        sort_col.as_deref(),
        sort_desc,
        &columns,
    )?;

    // Count filtered rows only when filters are active (avoids a full scan otherwise).
    // Sort and column projection don't affect row count, so counting on the full pipeline is fine.
    let total_rows = if filters.is_empty() {
        unfiltered_rows
    } else {
        datastore::count_lf(&lf)?
    };

    let df = lf
        .slice(offset, limit as u32)
        .collect()
        .map_err(|e| e.to_string())?;

    Ok(RowsResponse { rows: datastore::frame_to_rows(&df), total_rows })
}

/// Return all lat/lon coordinate pairs that pass the active filters and optional bounding box.
/// When `min_lat`/`max_lat`/`min_lon`/`max_lon` are all Some, only rows within that bbox
/// are returned. When all are None the full (filtered) dataset is returned so the frontend
/// can compute a fit-bounds extent on first load.
#[tauri::command]
fn get_map_points(
    lat_col: String,
    lon_col: String,
    filters: Vec<datastore::FilterSpec>,
    min_lat: Option<f64>,
    max_lat: Option<f64>,
    min_lon: Option<f64>,
    max_lon: Option<f64>,
    state: State<'_, AppState>,
) -> Result<Vec<datastore::MapPoint>, String> {
    let (file_path, schema) = state.loaded()?;
    // All four bounds present → cull to the viewport; otherwise return everything.
    let bbox = match (min_lat, max_lat, min_lon, max_lon) {
        (Some(a), Some(b), Some(c), Some(d)) => Some([a, b, c, d]),
        _ => None,
    };
    datastore::get_map_features(&file_path, &lat_col, &lon_col, &filters, &schema, bbox)
}

/// Return all H3 cell index values that pass the active filters as strings.
/// The frontend decodes each index to a polygon boundary using h3-js.
#[tauri::command]
fn get_h3_values(
    h3_col: String,
    filters: Vec<datastore::FilterSpec>,
    state: State<'_, AppState>,
) -> Result<Vec<datastore::H3Feature>, String> {
    let (file_path, schema) = state.loaded()?;
    datastore::get_h3_features(&file_path, &h3_col, &filters, &schema)
}

/// Return the filtered geometry column decoded from WKB into GeoJSON geometries,
/// each tagged with its source row index. The frontend wraps each into a Feature
/// and fetches the row's data lazily (via `get_row`) when it's clicked.
#[tauri::command]
fn get_geometry(
    geom_col: String,
    filters: Vec<datastore::FilterSpec>,
    state: State<'_, AppState>,
) -> Result<Vec<datastore::GeomFeature>, String> {
    let (file_path, schema) = state.loaded()?;
    datastore::get_geometry_features(&file_path, &geom_col, &filters, &schema)
}

/// Return a single source row (all columns) by its absolute file index, for the
/// map feature popup. Filters aren't reapplied: the index already identifies a
/// specific row the map is displaying.
#[tauri::command]
fn get_row(index: i64, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let file_path = state.loaded_path()?;
    datastore::get_row_at(&file_path, index)
}

#[tauri::command]
fn get_chart_data(
    x_col: String,
    y_cols: Vec<String>,
    filters: Vec<datastore::FilterSpec>,
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let (file_path, schema) = state.loaded()?;
    datastore::get_chart_rows(&file_path, &x_col, &y_cols, &filters, &schema)
}

/// Export the current view (with active sort, filters, and column selection) to a file.
/// Format is inferred from `dest`'s extension: `.parquet` → Parquet, else CSV.
#[tauri::command]
fn export_file(
    dest: String,
    sort_col: Option<String>,
    sort_desc: bool,
    filters: Vec<datastore::FilterSpec>,
    columns: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (file_path, schema) = state.loaded()?;
    let lf = datastore::build_pipeline(
        &file_path,
        &filters,
        &schema,
        sort_col.as_deref(),
        sort_desc,
        &columns,
    )?;
    let mut df = lf.collect().map_err(|e| e.to_string())?;
    datastore::write_file(&mut df, &dest)
}

/// Return the file path that was passed as a command-line argument at launch (e.g. via OS
/// file association), then clear it so subsequent calls return None.
#[tauri::command]
fn get_startup_file(state: State<'_, AppState>) -> Option<String> {
    state.startup_file.lock().unwrap().take()
}

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            file: Mutex::new(None),
            startup_file: Mutex::new(None),
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            if let Some(path) = args.get(1) {
                let lower = path.to_lowercase();
                if lower.ends_with(".csv") || lower.ends_with(".parquet") {
                    *app.state::<AppState>().startup_file.lock().unwrap() = Some(path.clone());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![load_file, get_rows, export_file, get_map_points, get_h3_values, get_geometry, get_row, get_chart_data, get_startup_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
