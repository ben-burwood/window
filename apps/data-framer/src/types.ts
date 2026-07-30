export type Dtype =
  | "integer" | "float" | "boolean" | "date" | "datetime" | "string"
  | "decimal" | "time" | "duration" | "categorical" | "binary"
  | "list" | "struct";

export interface ColumnInfo {
  name: string;
  dtype: Dtype;
}

export interface FileInfo {
  path: string;
  total_rows: number;
  columns: ColumnInfo[];
}

export interface FilterSpec {
  column: string;
  op: string;
  value: string;
  value2: string;
}

export interface RowsResponse {
  rows: Record<string, unknown>[];
  total_rows: number;
}

export interface ChartConfig {
  chartType: "line" | "scatter" | "bar";
  xColumn: string;
  yColumns: string[];
}

/**
 * Format a cell value for display: nested list/struct values (real JSON objects
 * from the backend) become compact JSON, null/undefined become "", everything
 * else is stringified. Shared by the data grid and the map feature popup.
 */
export function formatCellValue(v: unknown): string {
  return v != null && typeof v === "object" ? JSON.stringify(v) : String(v ?? "");
}
