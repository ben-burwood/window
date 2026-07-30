//! Converts a GeoParquet file into a GeoJSON `FeatureCollection` string.
//!
//! The frontend already renders any GeoJSON FeatureCollection, so the backend
//! just turns GeoParquet into that shape: geometry (WKB) is decoded with geozero
//! and the remaining columns become each feature's `properties` via arrow-json.
//!
//! Scope (kept deliberately small): only WKB-encoded geometry in WGS84 / CRS84,
//! which is the GeoParquet default and what MapLibre expects. Anything else
//! returns a clear error rather than pulling in heavy reprojection dependencies.

use std::fs::File;

use arrow_array::{cast::AsArray, Array, RecordBatch};
use arrow_schema::DataType;
use geozero::wkb::Wkb;
use geozero::ToJson;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde_json::Value;

pub fn to_geojson(path: &str) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {path}: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("{path} is not a valid Parquet file: {e}"))?;

    // The GeoParquet `geo` metadata tells us which column holds the geometry and
    // how it is encoded. Its absence means this isn't a GeoParquet file.
    let geo_raw = builder
        .metadata()
        .file_metadata()
        .key_value_metadata()
        .and_then(|kvs| kvs.iter().find(|kv| kv.key == "geo"))
        .and_then(|kv| kv.value.clone())
        .ok_or_else(|| "Not a GeoParquet file (missing 'geo' metadata).".to_string())?;
    let geo: Value = serde_json::from_str(&geo_raw)
        .map_err(|e| format!("Invalid GeoParquet 'geo' metadata: {e}"))?;

    let primary = geo
        .get("primary_column")
        .and_then(Value::as_str)
        .unwrap_or("geometry");
    let column_meta = geo
        .get("columns")
        .and_then(|c| c.get(primary))
        .ok_or_else(|| format!("GeoParquet metadata has no entry for geometry column '{primary}'."))?;

    let encoding = column_meta.get("encoding").and_then(Value::as_str).unwrap_or("");
    if !encoding.eq_ignore_ascii_case("WKB") {
        return Err(format!(
            "Unsupported GeoParquet geometry encoding '{encoding}'. Only WKB is supported."
        ));
    }
    ensure_wgs84(column_meta.get("crs"))?;

    let schema = builder.schema().clone();
    let geom_idx = schema
        .index_of(primary)
        .map_err(|_| format!("Geometry column '{primary}' not found in the Parquet schema."))?;
    let prop_indices: Vec<usize> = (0..schema.fields().len()).filter(|&i| i != geom_idx).collect();

    let reader = builder
        .build()
        .map_err(|e| format!("Failed to read Parquet file: {e}"))?;

    let mut out = String::from("{\"type\":\"FeatureCollection\",\"features\":[");
    let mut first = true;
    for batch in reader {
        let batch = batch.map_err(|e| format!("Failed to read Parquet data: {e}"))?;
        let properties = properties_json(&batch, &prop_indices)?;
        let geom_col = batch.column(geom_idx);
        for row in 0..batch.num_rows() {
            let Some(geometry) = wkb_to_geojson(geom_col.as_ref(), row)? else {
                continue; // null / empty geometry — skip the feature
            };
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str("{\"type\":\"Feature\",\"properties\":");
            out.push_str(&properties[row].to_string());
            out.push_str(",\"geometry\":");
            out.push_str(&geometry);
            out.push('}');
        }
    }
    out.push_str("]}");
    Ok(out)
}

/// Serialize every non-geometry column of a batch to one JSON object per row.
/// arrow-json omits null fields, which matches typical GeoJSON `properties`.
fn properties_json(batch: &RecordBatch, indices: &[usize]) -> Result<Vec<Value>, String> {
    if indices.is_empty() {
        return Ok(vec![Value::Object(Default::default()); batch.num_rows()]);
    }
    let sub = batch
        .project(indices)
        .map_err(|e| format!("Failed to project property columns: {e}"))?;
    let mut buf = Vec::new();
    let mut writer = arrow_json::ArrayWriter::new(&mut buf);
    writer
        .write(&sub)
        .map_err(|e| format!("Failed to encode feature properties: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("Failed to encode feature properties: {e}"))?;
    serde_json::from_slice(&buf).map_err(|e| format!("Failed to encode feature properties: {e}"))
}

/// Decode the WKB geometry at `row` into a GeoJSON geometry string, or `None`
/// when the value is null/empty.
fn wkb_to_geojson(col: &dyn Array, row: usize) -> Result<Option<String>, String> {
    if col.is_null(row) {
        return Ok(None);
    }
    let bytes: &[u8] = match col.data_type() {
        DataType::Binary => col.as_binary::<i32>().value(row),
        DataType::LargeBinary => col.as_binary::<i64>().value(row),
        other => {
            return Err(format!(
                "Geometry column has unsupported Arrow type {other:?}; expected binary WKB."
            ))
        }
    };
    if bytes.is_empty() {
        return Ok(None);
    }
    Wkb(bytes)
        .to_json()
        .map(Some)
        .map_err(|e| format!("Failed to decode WKB geometry: {e}"))
}

/// GeoParquet stores coordinates in longitude/latitude order, so any WGS84 /
/// CRS84 declaration renders correctly on MapLibre. A missing `crs` defaults to
/// CRS84 per the spec. Anything else needs reprojection and is rejected.
fn ensure_wgs84(crs: Option<&Value>) -> Result<(), String> {
    let crs = match crs {
        None | Some(Value::Null) => return Ok(()),
        Some(v) => v,
    };

    // A CRS can be a plain identifier string or a PROJJSON object.
    if let Some(s) = crs.as_str() {
        return if is_wgs84_name(s) {
            Ok(())
        } else {
            Err(crs_error(s))
        };
    }

    // PROJJSON: prefer the authoritative `id`, fall back to the human name.
    if let Some(id) = crs.get("id") {
        let authority = id.get("authority").and_then(Value::as_str).unwrap_or("");
        let code = id
            .get("code")
            .map(|c| c.as_str().map(str::to_string).unwrap_or_else(|| c.to_string()))
            .unwrap_or_default();
        if (authority.eq_ignore_ascii_case("EPSG") && code == "4326")
            || (authority.eq_ignore_ascii_case("OGC") && code.eq_ignore_ascii_case("CRS84"))
        {
            return Ok(());
        }
    }
    let name = crs.get("name").and_then(Value::as_str).unwrap_or("");
    if is_wgs84_name(name) {
        return Ok(());
    }
    Err(crs_error(if name.is_empty() {
        "(custom PROJJSON)"
    } else {
        name
    }))
}

fn is_wgs84_name(s: &str) -> bool {
    let normalized = s.to_uppercase().replace(' ', "");
    normalized.contains("WGS84") || normalized.contains("CRS84") || normalized.contains("EPSG:4326")
}

fn crs_error(what: &str) -> String {
    format!("Only WGS84 / CRS84 GeoParquet is supported (file declares CRS: {what}). Reproject to EPSG:4326 first.")
}
