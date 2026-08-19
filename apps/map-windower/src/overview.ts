import type { FeatureCollection, Feature, Geometry } from "geojson";

// A node in the structure tree.
// `children` being present (even if empty) marks the node as expandable; `detail` is muted secondary text shown after the label.
export interface OverviewNode {
  label: string;
  detail?: string;
  children?: OverviewNode[];
  defaultOpen?: boolean;
}

export const FEATURE_LIMIT = 2000;
const VALUE_PREVIEW_MAX = 40;

// Count leaf positions in a coordinates array of any depth.
function countPositions(coords: unknown): number {
  if (!Array.isArray(coords)) return 0;
  if (typeof coords[0] === "number") return 1;
  let total = 0;
  for (const c of coords) total += countPositions(c);
  return total;
}

function geometrySummary(geom: Geometry): string {
  switch (geom.type) {
    case "Point":
      return "1 position";
    case "MultiPoint":
    case "LineString":
      return `${countPositions(geom.coordinates)} positions`;
    case "MultiLineString":
      return `${geom.coordinates.length} lines · ${countPositions(geom.coordinates)} positions`;
    case "Polygon":
      return `${geom.coordinates.length} rings · ${countPositions(geom.coordinates)} positions`;
    case "MultiPolygon":
      return `${geom.coordinates.length} polygons · ${countPositions(geom.coordinates)} positions`;
    case "GeometryCollection":
      return `${geom.geometries.length} geometries`;
    default:
      return "";
  }
}

function geometryNode(geom: Geometry | null): OverviewNode {
  if (!geom) return { label: "geometry: null" };

  const node: OverviewNode = {
    label: `geometry: ${geom.type}`,
    detail: geometrySummary(geom),
  };
  if (geom.type === "GeometryCollection") {
    node.children = geom.geometries.map((g) => geometryNode(g));
  }
  return node;
}

// A short, type-annotated preview of a single property value.
// Never recurses: nested arrays/objects are shown as counts only.
function valuePreview(value: unknown): string {
  if (value === null) return "null (null)";
  if (typeof value === "string") {
    const text = value.length > VALUE_PREVIEW_MAX ? `${value.slice(0, VALUE_PREVIEW_MAX)}…` : value;
    return `"${text}" (string)`;
  }
  if (typeof value === "number") return `${value} (number)`;
  if (typeof value === "boolean") return `${value} (boolean)`;
  if (Array.isArray(value)) {
    return `[${value.length} item${value.length === 1 ? "" : "s"}] (array)`;
  }
  if (typeof value === "object") {
    const n = Object.keys(value as object).length;
    return `{${n} key${n === 1 ? "" : "s"}} (object)`;
  }
  return `${String(value)} (${typeof value})`;
}

function propertiesNode(props: Feature["properties"]): OverviewNode {
  const entries = props ? Object.entries(props) : [];
  return {
    label: "properties",
    detail: `{${entries.length}}`,
    children: entries.map(([key, value]) => ({
      label: key,
      detail: valuePreview(value),
    })),
  };
}

function featureNode(feature: Feature, index: number): OverviewNode {
  return {
    label: `[${index}] Feature`,
    children: [geometryNode(feature.geometry), propertiesNode(feature.properties)],
  };
}

// Turn a parsed FeatureCollection into the root of a collapsible structure tree.
export function buildOverview(fc: FeatureCollection): OverviewNode {
  const features = fc.features;
  const shown = features.slice(0, FEATURE_LIMIT).map((f, i) => featureNode(f, i));

  const hidden = features.length - shown.length;
  if (hidden > 0) {
    shown.push({ label: `… ${hidden} more features (not shown)` });
  }

  return {
    label: "FeatureCollection",
    detail: `(${features.length} feature${features.length === 1 ? "" : "s"})`,
    children: shown,
    defaultOpen: true,
  };
}
