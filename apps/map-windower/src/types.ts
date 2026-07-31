import type { FeatureCollection, Feature, Geometry } from "geojson";

// A loaded file is either parsed GeoJSON or a path to a PMTiles archive that the
// map reads directly via the pmtiles:// protocol.
export type LoadedSource =
  | { kind: "geojson"; name: string; data: FeatureCollection }
  | { kind: "pmtiles"; name: string; path: string };

export function fileKind(path: string): "geojson" | "pmtiles" | "geoparquet" {
  const lower = path.toLowerCase();
  if (lower.endsWith(".pmtiles")) return "pmtiles";
  if (lower.endsWith(".geoparquet")) return "geoparquet";
  return "geojson";
}

// Normalize any valid GeoJSON top-level object into a FeatureCollection so the
// map only ever has to deal with one shape.
export function toFeatureCollection(input: unknown): FeatureCollection {
  if (!input || typeof input !== "object") {
    throw new Error("File does not contain a valid GeoJSON object.");
  }
  const obj = input as { type?: string };

  switch (obj.type) {
    case "FeatureCollection":
      return input as FeatureCollection;
    case "Feature":
      return { type: "FeatureCollection", features: [input as Feature] };
    case "Point":
    case "MultiPoint":
    case "LineString":
    case "MultiLineString":
    case "Polygon":
    case "MultiPolygon":
    case "GeometryCollection":
      return {
        type: "FeatureCollection",
        features: [{ type: "Feature", properties: {}, geometry: input as Geometry }],
      };
    default:
      throw new Error(`Unsupported GeoJSON type: ${obj.type ?? "(missing 'type' field)"}`);
  }
}
