import { setWorkerUrl } from "maplibre-gl";
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url";

// MapLibre 6 can't locate its worker inside a bundler's module graph, so point it
// at the Vite-bundled worker once, before any map is created.
setWorkerUrl(workerUrl);
