use polars::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub dtype: String,
}

/// Persisted across `get_rows` calls so we don't re-read the file header each time.
#[derive(Debug)]
pub struct LoadedFile {
    pub path: String,
    pub total_rows: usize,
    pub schema: Vec<ColumnInfo>,
}

/// Tauri managed state — wrapped in Mutex so commands can mutate it.
pub struct AppState {
    pub file: Mutex<Option<LoadedFile>>,
    pub startup_file: Mutex<Option<String>>,
}

impl AppState {
    /// Path and schema of the currently loaded file, erroring if none is open.
    /// Cloned out so the lock isn't held across the subsequent file scan/collect.
    pub fn loaded(&self) -> Result<(String, Vec<ColumnInfo>), String> {
        let guard = self.file.lock().unwrap();
        let f = guard.as_ref().ok_or("No file loaded")?;
        Ok((f.path.clone(), f.schema.clone()))
    }

    /// Path of the currently loaded file, erroring if none is open.
    pub fn loaded_path(&self) -> Result<String, String> {
        let guard = self.file.lock().unwrap();
        Ok(guard.as_ref().ok_or("No file loaded")?.path.clone())
    }
}

/// Returned by `load_file` command.
#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub total_rows: usize,
    pub columns: Vec<ColumnInfo>,
}

/// Returned by `get_rows` command.
#[derive(Debug, Serialize)]
pub struct RowsResponse {
    pub rows: Vec<serde_json::Value>,
    pub total_rows: usize,
}

// Map features carry only the original file row index (`idx`), not the row data
// itself — the frontend fetches a clicked row's full values lazily via `get_row`.

/// A lat/lon point tagged with its source row index.
#[derive(Debug, Serialize)]
pub struct MapPoint {
    pub lat: f64,
    pub lon: f64,
    pub idx: IdxSize,
}

/// An H3 cell index tagged with its source row index.
#[derive(Debug, Serialize)]
pub struct H3Feature {
    pub cell: String,
    pub idx: IdxSize,
}

/// A decoded GeoJSON geometry tagged with its source row index.
#[derive(Debug, Serialize)]
pub struct GeomFeature {
    pub geometry: serde_json::Value,
    pub idx: IdxSize,
}

/// Name of the synthetic row-index column added before filtering so every map
/// feature can be traced back to its absolute position in the source file.
pub const ROW_IDX: &str = "__row_idx";

// ---------------------------------------------------------------------------
// File scanning
// ---------------------------------------------------------------------------

/// Build a LazyFrame from a file path. Supports .parquet and .csv.
pub fn scan_file(path: &str) -> Result<LazyFrame, String> {
    match file_ext(path).as_str() {
        "parquet" => {
            LazyFrame::scan_parquet(path.into(), ScanArgsParquet::default())
                .map_err(|e| e.to_string())
        }
        "csv" => {
            LazyCsvReader::new(path.into()).finish().map_err(|e| e.to_string())
        }
        other => Err(format!("Unsupported file format: .{other}")),
    }
}

/// Row count from an already-scanned LazyFrame, avoiding a second file open.
///   Parquet: `len()` reads row-group footer metadata only (no row data loaded).
///   CSV: counts newlines minus the header row (streaming, no data heap alloc).
pub fn count_rows(lf: LazyFrame, path: &str) -> Result<usize, String> {
    match file_ext(path).as_str() {
        "parquet" => count_lf(&lf),
        _ => {
            use std::io::BufRead;
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            Ok(std::io::BufReader::new(file)
                .lines()
                .count()
                .saturating_sub(1))
        }
    }
}

/// Aggregate `len()` over a LazyFrame — used for both unfiltered parquet counts
/// and filtered row counts where line-counting is not applicable.
pub(crate) fn count_lf(lf: &LazyFrame) -> Result<usize, String> {
    lf.clone().select([len().alias("_n")])
        .collect()
        .map_err(|e| e.to_string())?
        .column("_n")
        .map_err(|e| e.to_string())?
        .get(0)
        .map_err(|e| e.to_string())?
        .try_extract::<usize>()
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Schema helpers
// ---------------------------------------------------------------------------

/// Extract column metadata from a LazyFrame without collecting any data.
pub fn extract_schema(lf: &mut LazyFrame) -> Result<Vec<ColumnInfo>, String> {
    // collect_schema() reads file metadata only (no rows loaded).
    let schema = lf.collect_schema().map_err(|e| e.to_string())?;
    Ok(schema
        .iter()
        .map(|(name, dtype)| ColumnInfo {
            name: name.to_string(),
            dtype: dtype_to_str(dtype).to_string(),
        })
        .collect())
}

/// Map a Polars DataType to a simple string the frontend can use for column typing.
pub fn dtype_to_str(dtype: &DataType) -> &'static str {
    match dtype {
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => "integer",
        DataType::Float32 | DataType::Float64 => "float",
        DataType::Boolean => "boolean",
        DataType::Date => "date",
        DataType::Datetime(_, _) => "datetime",
        DataType::Decimal(..) => "decimal",
        DataType::Time => "time",
        DataType::Duration(..) => "duration",
        DataType::Categorical(..) | DataType::Enum(..) => "categorical",
        DataType::Binary | DataType::BinaryOffset => "binary",
        DataType::List(..) | DataType::Array(..) => "list",
        DataType::Struct(..) => "struct",
        // Arrow extension types (e.g. GeoParquet's `geoarrow.wkb`) are unwrapped to
        // their underlying storage type — a WKB geometry column reads back as binary.
        DataType::Extension(_, storage) => dtype_to_str(storage),
        _ => "string",
    }
}

// ---------------------------------------------------------------------------
// DataFrame → JSON rows
// ---------------------------------------------------------------------------

/// Convert a (small, paginated) DataFrame into a Vec of JSON objects
/// suitable for serialisation over Tauri IPC.
pub fn frame_to_rows(df: &DataFrame) -> Vec<serde_json::Value> {
    let columns = df.columns();
    // Pre-extract column names once so the inner loop doesn't allocate per cell.
    let names: Vec<String> = columns.iter().map(|c| c.name().to_string()).collect();
    let height = df.height();
    let mut rows = Vec::with_capacity(height);
    for i in 0..height {
        let mut map = serde_json::Map::with_capacity(columns.len());
        for (col, name) in columns.iter().zip(&names) {
            let val: serde_json::Value = col
                .get(i)
                .map(anyvalue_to_json)
                .unwrap_or(serde_json::Value::Null);
            map.insert(name.clone(), val);
        }
        rows.push(serde_json::Value::Object(map));
    }
    rows
}

fn anyvalue_to_json(v: AnyValue<'_>) -> serde_json::Value {
    match v {
        AnyValue::Null => serde_json::Value::Null,
        AnyValue::Boolean(b) => serde_json::Value::Bool(b),
        AnyValue::Int8(n) => n.into(),
        AnyValue::Int16(n) => n.into(),
        AnyValue::Int32(n) => n.into(),
        AnyValue::Int64(n) => n.into(),
        AnyValue::UInt8(n) => n.into(),
        AnyValue::UInt16(n) => n.into(),
        AnyValue::UInt32(n) => n.into(),
        AnyValue::UInt64(n) => n.into(),
        AnyValue::Float32(f) => serde_json::Number::from_f64(f as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        AnyValue::Float64(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        AnyValue::String(s) => serde_json::Value::String(s.to_string()),
        AnyValue::StringOwned(s) => serde_json::Value::String(s.to_string()),
        AnyValue::Decimal(v, _precision, scale) => serde_json::Number::from_f64(v as f64 / 10f64.powi(scale as i32))
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        // Binary blobs: emit a short descriptor rather than raw bytes.
        AnyValue::Binary(bytes) => serde_json::Value::String(format!("<{} bytes>", bytes.len())),
        AnyValue::BinaryOwned(bytes) => serde_json::Value::String(format!("<{} bytes>", bytes.len())),
        // Nested collections → real JSON arrays, recursing per element.
        AnyValue::List(series) => list_to_json(&series),
        AnyValue::Array(series, _) => list_to_json(&series),
        // Structs → JSON objects, preserving field names (the key win over Display).
        AnyValue::Struct(_, _, fields) => struct_to_json(&v, fields.iter().map(|f| f.name().to_string())),
        AnyValue::StructOwned(ref payload) => {
            let names: Vec<String> = payload.1.iter().map(|f| f.name().to_string()).collect();
            struct_to_json(&v, names.into_iter())
        }
        other => serde_json::Value::String(format!("{other}")),
    }
}

/// Convert a Polars Series (the inner values of a List/Array cell) to a JSON array.
fn list_to_json(series: &Series) -> serde_json::Value {
    serde_json::Value::Array(series.iter().map(anyvalue_to_json).collect())
}

/// Convert a struct AnyValue to a JSON object by zipping field names with values.
fn struct_to_json(v: &AnyValue<'_>, names: impl Iterator<Item = String>) -> serde_json::Value {
    let mut values = Vec::new();
    v._materialize_struct_av(&mut values);
    let map: serde_json::Map<String, serde_json::Value> = names
        .zip(values)
        .map(|(name, av)| (name, anyvalue_to_json(av)))
        .collect();
    serde_json::Value::Object(map)
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn file_ext(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct FilterSpec {
    pub column: String,
    pub op: String,
    pub value: Option<String>,
    pub value2: Option<String>,
}

/// AND-combine all filters and apply them to the LazyFrame.
/// Returns the original LazyFrame unchanged when `filters` is empty.
pub fn apply_filters(
    lf: LazyFrame,
    filters: &[FilterSpec],
    schema: &[ColumnInfo],
) -> Result<LazyFrame, String> {
    if filters.is_empty() {
        return Ok(lf);
    }

    let mut combined: Option<Expr> = None;
    for spec in filters {
        let dtype = schema
            .iter()
            .find(|c| c.name == spec.column)
            .map(|c| c.dtype.as_str())
            .ok_or_else(|| format!("Unknown column: {}", spec.column))?;
        let expr = build_filter_expr(spec, dtype)?;
        combined = Some(match combined {
            None => expr,
            Some(prev) => prev.and(expr),
        });
    }

    Ok(lf.filter(combined.expect("loop ran at least once; combined is Some")))
}

fn build_filter_expr(spec: &FilterSpec, dtype: &str) -> Result<Expr, String> {
    let c = spec.column.as_str();

    // Null checks apply to all dtypes and need no value
    match spec.op.as_str() {
        "is_null" => return Ok(col(c).is_null()),
        "is_not_null" => return Ok(col(c).is_not_null()),
        "is_true" => return Ok(col(c).eq(lit(true))),
        "is_false" => return Ok(col(c).eq(lit(false))),
        _ => {}
    }

    let v = spec.value.as_deref().ok_or("Missing filter value")?;

    match spec.op.as_str() {
        "eq" => Ok(col(c).eq(parse_value(v, dtype)?)),
        "neq" => Ok(col(c).neq(parse_value(v, dtype)?)),
        "gt" => Ok(col(c).gt(parse_value(v, dtype)?)),
        "gte" => Ok(col(c).gt_eq(parse_value(v, dtype)?)),
        "lt" => Ok(col(c).lt(parse_value(v, dtype)?)),
        "lte" => Ok(col(c).lt_eq(parse_value(v, dtype)?)),
        "between" => {
            let v2 = spec
                .value2
                .as_deref()
                .ok_or("Missing second value for 'between'")?;
            Ok(col(c)
                .gt_eq(parse_value(v, dtype)?)
                .and(col(c).lt_eq(parse_value(v2, dtype)?)))
        }
        "contains" => Ok(col(c).str().contains_literal(lit(v))),
        "not_contains" => Ok(col(c).str().contains_literal(lit(v)).not()),
        "starts_with" => Ok(col(c).str().starts_with(lit(v))),
        "ends_with" => Ok(col(c).str().ends_with(lit(v))),
        other => Err(format!("Unknown filter op: {other}")),
    }
}

// ---------------------------------------------------------------------------
// File writing
// ---------------------------------------------------------------------------

/// Write a DataFrame to disk. Format is inferred from the file extension:
/// `.parquet` → Parquet, anything else → CSV.
pub fn write_file(df: &mut DataFrame, path: &str) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    match file_ext(path).as_str() {
        "parquet" => ParquetWriter::new(file)
            .finish(df)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        _ => CsvWriter::new(file)
            .finish(df)
            .map_err(|e| e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Column selection
// ---------------------------------------------------------------------------

/// Project the LazyFrame to only the requested columns.
/// Empty slice = no-op (all columns pass through).
/// Call after sort so a hidden sort column doesn't cause an error.
pub fn apply_column_select(lf: LazyFrame, columns: &[String]) -> LazyFrame {
    if columns.is_empty() {
        return lf;
    }
    lf.select(columns.iter().map(|c| col(c.as_str())).collect::<Vec<_>>())
}

fn parse_value(v: &str, dtype: &str) -> Result<Expr, String> {
    match dtype {
        "integer" => {
            let n = v
                .parse::<i64>()
                .map_err(|e| format!("Cannot parse '{v}' as integer: {e}"))?;
            Ok(lit(n))
        }
        "float" | "decimal" => {
            let f = v
                .parse::<f64>()
                .map_err(|e| format!("Cannot parse '{v}' as {dtype}: {e}"))?;
            Ok(lit(f))
        }
        "date" => {
            let date = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d")
                .map_err(|e| format!("Cannot parse '{v}' as date (expected YYYY-MM-DD): {e}"))?;
            let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
            let days = date.signed_duration_since(epoch).num_days() as i32;
            Ok(lit(days).cast(DataType::Date))
        }
        "datetime" => {
            let dt = chrono::NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S")
                .map_err(|e| format!("Cannot parse '{v}' as datetime (expected YYYY-MM-DDTHH:MM:SS): {e}"))?;
            let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let us = dt
                .signed_duration_since(epoch)
                .num_microseconds()
                .ok_or("Datetime overflow")?;
            Ok(lit(us).cast(DataType::Datetime(TimeUnit::Microseconds, None)))
        }
        // string and everything else: literal string comparison
        _ => Ok(lit(v)),
    }
}

// ---------------------------------------------------------------------------
// Chart data
// ---------------------------------------------------------------------------

pub fn get_chart_rows(
    path: &str,
    x_col: &str,
    y_cols: &[String],
    filters: &[FilterSpec],
    schema: &[ColumnInfo],
) -> Result<Vec<serde_json::Value>, String> {
    let cols: Vec<String> = std::iter::once(x_col.to_string())
        .chain(y_cols.iter().cloned())
        .collect();
    let lf = build_pipeline(path, filters, schema, Some(x_col), false, &cols)?;
    let df = lf.collect().map_err(|e| e.to_string())?;
    Ok(frame_to_rows(&df))
}

// ---------------------------------------------------------------------------
// Geometry (GeoParquet)
// ---------------------------------------------------------------------------

/// Shared prefix for the three map-feature queries: scan the file, tag every row
/// with its absolute pre-filter index (`ROW_IDX`), then apply the active filters.
fn map_scan(
    path: &str,
    filters: &[FilterSpec],
    schema: &[ColumnInfo],
) -> Result<LazyFrame, String> {
    let lf = scan_file(path)?.with_row_index(ROW_IDX, None);
    apply_filters(lf, filters, schema)
}

/// Read the `ROW_IDX` column of a collected map-feature frame as an index array.
fn row_idx(df: &DataFrame) -> Result<&IdxCa, String> {
    df.column(ROW_IDX)
        .and_then(|c| c.idx())
        .map_err(|e| e.to_string())
}

/// Filtered lat/lon points, each tagged with its source row index. When `bbox`
/// (`[min_lat, max_lat, min_lon, max_lon]`) is given, only points inside it are
/// returned. Rows with a null coordinate are dropped.
pub fn get_map_features(
    path: &str,
    lat_col: &str,
    lon_col: &str,
    filters: &[FilterSpec],
    schema: &[ColumnInfo],
    bbox: Option<[f64; 4]>,
) -> Result<Vec<MapPoint>, String> {
    let mut lf = map_scan(path, filters, schema)?;
    if let Some([min_lat, max_lat, min_lon, max_lon]) = bbox {
        lf = lf.filter(
            col(lat_col)
                .gt_eq(lit(min_lat))
                .and(col(lat_col).lt_eq(lit(max_lat)))
                .and(col(lon_col).gt_eq(lit(min_lon)))
                .and(col(lon_col).lt_eq(lit(max_lon))),
        );
    }

    let df = lf
        .select([col(ROW_IDX), col(lat_col), col(lon_col)])
        .collect()
        .map_err(|e| e.to_string())?;

    let idx = row_idx(&df)?;
    let lat_s = df
        .column(lat_col)
        .map_err(|e| e.to_string())?
        .cast(&DataType::Float64)
        .map_err(|e| e.to_string())?;
    let lon_s = df
        .column(lon_col)
        .map_err(|e| e.to_string())?
        .cast(&DataType::Float64)
        .map_err(|e| e.to_string())?;
    let lats = lat_s.f64().map_err(|e| e.to_string())?;
    let lons = lon_s.f64().map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(df.height());
    for ((row, lat), lon) in idx.into_iter().zip(lats.iter()).zip(lons.iter()) {
        if let (Some(row), Some(lat), Some(lon)) = (row, lat, lon) {
            out.push(MapPoint { lat, lon, idx: row });
        }
    }
    Ok(out)
}

/// Filtered H3 cell features, each tagged with its source row index. Rows with a
/// null cell value are dropped.
pub fn get_h3_features(
    path: &str,
    h3_col: &str,
    filters: &[FilterSpec],
    schema: &[ColumnInfo],
) -> Result<Vec<H3Feature>, String> {
    let df = map_scan(path, filters, schema)?
        .select([col(ROW_IDX), col(h3_col)])
        .collect()
        .map_err(|e| e.to_string())?;

    let idx = row_idx(&df)?;
    let cells = df
        .column(h3_col)
        .map_err(|e| e.to_string())?
        .cast(&DataType::String)
        .map_err(|e| e.to_string())?;
    let cells = cells.str().map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(df.height());
    for (row, cell) in idx.into_iter().zip(cells.into_iter()) {
        if let (Some(row), Some(cell)) = (row, cell) {
            out.push(H3Feature { cell: cell.to_string(), idx: row });
        }
    }
    Ok(out)
}

/// Decode the (filtered) WKB geometry column into GeoJSON geometries, each tagged
/// with its source row index. Null and unparseable rows are skipped so a single
/// bad blob can't blank the whole map. Coordinates are emitted verbatim
/// (GeoParquet geometry is stored lon/lat, which is also GeoJSON order).
pub fn get_geometry_features(
    path: &str,
    geom_col: &str,
    filters: &[FilterSpec],
    schema: &[ColumnInfo],
) -> Result<Vec<GeomFeature>, String> {
    let df = map_scan(path, filters, schema)?
        .select([col(ROW_IDX), col(geom_col)])
        .collect()
        .map_err(|e| e.to_string())?;

    let idx = row_idx(&df)?;
    // The column arrives as an Arrow extension type (geoarrow.wkb) which can't be
    // `.cast()`, but its AnyValues already surface as Binary storage — read those.
    let column = df.column(geom_col).map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(column.len());
    for i in 0..column.len() {
        // Decode within the match: BinaryOwned's bytes live only as long as the
        // AnyValue temporary, so they can't escape the arm.
        let geom = match column.get(i) {
            Ok(AnyValue::Binary(b)) => wkb_to_geojson(b).ok(),
            Ok(AnyValue::BinaryOwned(b)) => wkb_to_geojson(&b).ok(),
            _ => None,
        };
        if let (Some(geometry), Some(row)) = (geom, idx.get(i)) {
            out.push(GeomFeature { geometry, idx: row });
        }
    }
    Ok(out)
}

/// Fetch a single source row by its absolute file position, as a JSON object.
/// Uses a positional slice so parquet readers can skip untouched row groups
/// instead of scanning the whole file per popup.
pub fn get_row_at(path: &str, index: i64) -> Result<serde_json::Value, String> {
    let df = scan_file(path)?
        .slice(index, 1)
        .collect()
        .map_err(|e| e.to_string())?;
    let mut rows = frame_to_rows(&df);
    Ok(rows.pop().unwrap_or(serde_json::Value::Null))
}

/// Convert a single WKB blob into a GeoJSON geometry `serde_json::Value`.
fn wkb_to_geojson(bytes: &[u8]) -> Result<serde_json::Value, String> {
    use geozero::ToJson;
    let json = geozero::wkb::Wkb(bytes)
        .to_json()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Shared query pipeline
// ---------------------------------------------------------------------------

/// Build a ready-to-collect LazyFrame from the common query parameters shared
/// by `get_rows` and `export_file`: scan → filter → sort → column projection.
pub fn build_pipeline(
    path: &str,
    filters: &[FilterSpec],
    schema: &[ColumnInfo],
    sort_col: Option<&str>,
    sort_desc: bool,
    columns: &[String],
) -> Result<LazyFrame, String> {
    let lf = scan_file(path)?;
    let lf = apply_filters(lf, filters, schema)?;
    let lf = match sort_col {
        Some(c) => lf.sort([c], SortMultipleOptions::default().with_order_descending(sort_desc)),
        None => lf,
    };
    Ok(apply_column_select(lf, columns))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_serializes_with_field_names() {
        let df = df!["a" => [1i64, 2], "b" => ["x", "y"]].unwrap();
        let st = df
            .lazy()
            .select([as_struct(vec![col("a"), col("b")]).alias("st")])
            .collect()
            .unwrap();
        assert_eq!(dtype_to_str(st.column("st").unwrap().dtype()), "struct");
        let rows = frame_to_rows(&st);
        assert_eq!(rows[0]["st"], serde_json::json!({ "a": 1, "b": "x" }));
    }

    #[test]
    fn list_serializes_as_json_array() {
        let lf = df!["x" => [1i64, 2, 3]]
            .unwrap()
            .lazy()
            .select([col("x").implode().alias("xs")])
            .collect()
            .unwrap();
        assert_eq!(dtype_to_str(lf.column("xs").unwrap().dtype()), "list");
        let rows = frame_to_rows(&lf);
        assert_eq!(rows[0]["xs"], serde_json::json!([1, 2, 3]));
    }

    /// End-to-end: write a Parquet file with nested columns, then read it back
    /// through the real scan → schema → collect → frame_to_rows path. Guards
    /// against Arrow yielding differently-shaped AnyValues than lazy-built ones.
    #[test]
    fn nested_parquet_round_trip() {
        let mut df = df!["a" => [1i64, 2], "b" => ["x", "y"]]
            .unwrap()
            .lazy()
            .select([
                as_struct(vec![col("a"), col("b")]).alias("st"),
                col("a").implode().over([lit(1)]).alias("xs"),
            ])
            .collect()
            .unwrap();

        let path = std::env::temp_dir().join("data_framer_nested_test.parquet");
        let path_str = path.to_str().unwrap();
        write_file(&mut df, path_str).unwrap();

        let mut lf = scan_file(path_str).unwrap();
        let schema = extract_schema(&mut lf).unwrap();
        let dtypes: std::collections::HashMap<_, _> =
            schema.iter().map(|c| (c.name.as_str(), c.dtype.as_str())).collect();
        assert_eq!(dtypes["st"], "struct");
        assert_eq!(dtypes["xs"], "list");

        let out = lf.collect().unwrap();
        let rows = frame_to_rows(&out);
        assert_eq!(rows[0]["st"], serde_json::json!({ "a": 1, "b": "x" }));
        assert_eq!(rows[0]["xs"][0], serde_json::json!(1));

        std::fs::remove_file(path).ok();
    }
}
