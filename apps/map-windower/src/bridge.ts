// map-windower's typed IPC wrappers. The only place this app's invoke() string literals live;
// built on the shared @window/bridge. Components import from here, never call invoke directly.
import { invoke } from "@window/bridge";

export {
  getStartupFile,
  openFile,
  onFileDrop,
  onOpenFile,
  watchFile,
  onFileChanged,
} from "@window/bridge";

/** Read a GeoJSON file and return its raw text (the frontend parses/validates it). */
export function loadGeojsonText(path: string): Promise<string> {
  return invoke<string>("load_file", { path });
}

/** Read a GeoParquet file and return it as a GeoJSON FeatureCollection string. */
export function loadGeoparquetText(path: string): Promise<string> {
  return invoke<string>("load_geoparquet", { path });
}
