// data-framer's typed IPC wrappers. The only place this app's invoke() string literals live;
// built on the shared @window/bridge. Components import from here, never call invoke directly.
import { invoke } from "@window/bridge";
import type { FileInfo, FilterSpec, RowsResponse } from "./types";

export {
  getStartupFile,
  openFile,
  saveFile,
  onFileDrop,
  onOpenFile,
  watchFile,
  onFileChanged,
} from "@window/bridge";

// ---------------------------------------------------------------------------
// Map-feature shapes returned by the map commands. Each carries `idx` — the
// row's absolute position in the source file — so a click can fetch just that
// row's data (see getRow).
// ---------------------------------------------------------------------------
export interface MapPoint {
  lat: number;
  lon: number;
  idx: number;
}
export interface H3Feature {
  cell: string;
  idx: number;
}
export interface GeomFeature {
  geometry: GeoJSON.Geometry;
  idx: number;
}

/** Open a Parquet/CSV file and read its schema + row count. */
export function loadFile(path: string) {
  return invoke<FileInfo>("load_file", { path });
}

/** A page of rows for the infinite-scroll grid. */
export type GetRowsArgs = {
  offset: number;
  limit: number;
  sortCol: string | null;
  sortDesc: boolean;
  filters: FilterSpec[];
  columns: string[];
};
export function getRows(args: GetRowsArgs) {
  return invoke<RowsResponse>("get_rows", args);
}

/** Fetch the raw rows powering a chart (one x column, one or more y columns). */
export type GetChartDataArgs = {
  xCol: string;
  yCols: string[];
  filters: FilterSpec[];
};
export function getChartData(args: GetChartDataArgs) {
  return invoke<Record<string, unknown>[]>("get_chart_data", args);
}

/** Fetch lat/lon map points, optionally clipped to a bounding box. */
export type GetMapPointsArgs = {
  latCol: string | null;
  lonCol: string | null;
  filters: FilterSpec[];
  minLat: number | null;
  maxLat: number | null;
  minLon: number | null;
  maxLon: number | null;
};
export function getMapPoints(args: GetMapPointsArgs) {
  return invoke<MapPoint[]>("get_map_points", args);
}

/** Fetch H3 cell indices for the map. */
export type GetH3ValuesArgs = {
  h3Col: string | null;
  filters: FilterSpec[];
};
export function getH3Values(args: GetH3ValuesArgs) {
  return invoke<H3Feature[]>("get_h3_values", args);
}

/** Fetch decoded GeoJSON geometries (from WKB) for the map. */
export type GetGeometryArgs = {
  geomCol: string | null;
  filters: FilterSpec[];
};
export function getGeometry(args: GetGeometryArgs) {
  return invoke<GeomFeature[]>("get_geometry", args);
}

/** Fetch a single source row by its absolute index (for map feature popups). */
export function getRow(index: number) {
  return invoke<Record<string, unknown> | null>("get_row", { index });
}

/** Export the current view (filters + sort + column selection) to CSV/Parquet. */
export type ExportFileArgs = {
  dest: string;
  sortCol: string | null;
  sortDesc: boolean;
  filters: FilterSpec[];
  columns: string[];
};
export function exportFile(args: ExportFileArgs) {
  return invoke<void>("export_file", args);
}
