mod datastore;

use datastore::{AppState, FileInfo, LoadedFile, RowsResponse};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Emitted (payload: file path) when a query first needs lossy UTF-8 decoding.
const LOSSY_UTF8_EVENT: &str = "lossy-utf8";

/// Run a query against the currently loaded file, erroring if none is open.
/// The file is cloned out so the lock isn't held across the scan/collect.
/// If the query hits invalid UTF-8, the stored file is switched to lossy
/// decoding (so later queries start lossy), the frontend is told, and the
/// query is retried.
fn query<T>(app: &AppHandle, f: impl Fn(&LoadedFile) -> Result<T, String>) -> Result<T, String> {
    let state = app.state::<AppState>();
    let mut file = state.file.lock().unwrap().clone().ok_or("No file loaded")?;
    match f(&file) {
        Err(e) if !file.source.lossy_utf8 && datastore::is_utf8_error(&e) => {
            file.source.lossy_utf8 = true;
            // Only flag the file we ran against, in case another was loaded meanwhile.
            if let Some(stored) = state.file.lock().unwrap().as_mut() {
                if stored.source.path == file.source.path {
                    stored.source.lossy_utf8 = true;
                }
            }
            let _ = app.emit(LOSSY_UTF8_EVENT, &file.source.path);
            f(&file)
        }
        r => r,
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Load a parquet, CSV or TSV file: reads schema + row count (no data), stores
/// the file path in managed state, and returns metadata to the frontend.
#[tauri::command]
fn load_file(path: String, state: State<'_, AppState>) -> Result<FileInfo, String> {
    let mut source = datastore::Source::new(&path);
    let (columns, total_rows) = datastore::with_utf8_fallback(&mut source, |source| {
        let mut lf = source.scan()?;
        let columns = datastore::extract_schema(&mut lf)?;
        Ok((columns, datastore::count_rows(&lf)?))
    })?;
    let lossy_utf8 = source.lossy_utf8;

    *state.file.lock().unwrap() = Some(LoadedFile {
        source,
        total_rows,
        schema: columns.clone(),
    });

    Ok(FileInfo {
        path,
        total_rows,
        columns,
        lossy_utf8,
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
    app: AppHandle,
) -> Result<RowsResponse, String> {
    query(&app, |file| {
        let lf = datastore::build_pipeline(
            &file.source,
            &filters,
            &file.schema,
            sort_col.as_deref(),
            sort_desc,
            &columns,
        )?;

        // Count filtered rows only when filters are active (avoids a full scan otherwise).
        // Sort and column projection don't affect row count, so counting on the full pipeline is fine.
        let total_rows = if filters.is_empty() {
            file.total_rows
        } else {
            datastore::count_lf(&lf)?
        };

        let df = lf
            .slice(offset, limit as u32)
            .collect()
            .map_err(|e| e.to_string())?;

        Ok(RowsResponse {
            rows: datastore::frame_to_rows(&df),
            total_rows,
        })
    })
}

/// Return all lat/lon coordinate pairs that pass the active filters and optional bounding box.
/// When `min_lat`/`max_lat`/`min_lon`/`max_lon` are all Some, only rows within that bbox
/// are returned. When all are None the full (filtered) dataset is returned so the frontend
/// can compute a fit-bounds extent on first load.
// A Tauri command whose parameters map 1:1 to a frontend call; bundling them into a struct
// would only move the shape into the bridge without simplifying anything.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn get_map_points(
    lat_col: String,
    lon_col: String,
    filters: Vec<datastore::FilterSpec>,
    min_lat: Option<f64>,
    max_lat: Option<f64>,
    min_lon: Option<f64>,
    max_lon: Option<f64>,
    app: AppHandle,
) -> Result<Vec<datastore::MapPoint>, String> {
    // All four bounds present → cull to the viewport; otherwise return everything.
    let bbox = match (min_lat, max_lat, min_lon, max_lon) {
        (Some(a), Some(b), Some(c), Some(d)) => Some([a, b, c, d]),
        _ => None,
    };
    query(&app, |file| {
        datastore::get_map_features(
            &file.source,
            &lat_col,
            &lon_col,
            &filters,
            &file.schema,
            bbox,
        )
    })
}

/// Return all H3 cell index values that pass the active filters as strings.
/// The frontend decodes each index to a polygon boundary using h3-js.
#[tauri::command]
fn get_h3_values(
    h3_col: String,
    filters: Vec<datastore::FilterSpec>,
    app: AppHandle,
) -> Result<Vec<datastore::H3Feature>, String> {
    query(&app, |file| {
        datastore::get_h3_features(&file.source, &h3_col, &filters, &file.schema)
    })
}

/// Return the filtered geometry column decoded from WKB into GeoJSON geometries,
/// each tagged with its source row index. The frontend wraps each into a Feature
/// and fetches the row's data lazily (via `get_row`) when it's clicked.
#[tauri::command]
fn get_geometry(
    geom_col: String,
    filters: Vec<datastore::FilterSpec>,
    app: AppHandle,
) -> Result<Vec<datastore::GeomFeature>, String> {
    query(&app, |file| {
        datastore::get_geometry_features(&file.source, &geom_col, &filters, &file.schema)
    })
}

/// Return a single source row (all columns) by its absolute file index, for the
/// map feature popup. Filters aren't reapplied: the index already identifies a
/// specific row the map is displaying.
#[tauri::command]
fn get_row(index: i64, app: AppHandle) -> Result<serde_json::Value, String> {
    query(&app, |file| datastore::get_row_at(&file.source, index))
}

#[tauri::command]
fn get_chart_data(
    x_col: String,
    y_cols: Vec<String>,
    filters: Vec<datastore::FilterSpec>,
    app: AppHandle,
) -> Result<Vec<serde_json::Value>, String> {
    query(&app, |file| {
        datastore::get_chart_rows(&file.source, &x_col, &y_cols, &filters, &file.schema)
    })
}

/// Export the current view (with active sort, filters, and column selection) to a file.
/// Format is inferred from `dest`'s extension: `.parquet` → Parquet, `.tsv` → TSV, else CSV.
#[tauri::command]
fn export_file(
    dest: String,
    sort_col: Option<String>,
    sort_desc: bool,
    filters: Vec<datastore::FilterSpec>,
    columns: Vec<String>,
    app: AppHandle,
) -> Result<(), String> {
    // Collect inside the query (so a UTF-8 failure can retry) but write outside it,
    // so a retry never rewrites a half-written file.
    let mut df = query(&app, |file| {
        datastore::build_pipeline(
            &file.source,
            &filters,
            &file.schema,
            sort_col.as_deref(),
            sort_desc,
            &columns,
        )?
        .collect()
        .map_err(|e| e.to_string())
    })?;
    datastore::write_file(&mut df, &dest)
}

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    window_tauri::run(
        window_tauri::app(&["csv", "tsv", "parquet"])
            .manage(AppState {
                file: Mutex::new(None),
            })
            .invoke_handler(tauri::generate_handler![
                load_file,
                get_rows,
                export_file,
                get_map_points,
                get_h3_values,
                get_geometry,
                get_row,
                get_chart_data,
                window_tauri::get_startup_file,
                window_tauri::watch_file
            ]),
        tauri::generate_context!(),
    );
}
