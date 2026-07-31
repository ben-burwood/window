<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import maplibregl, { type MapGeoJSONFeature } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { PMTiles, Protocol, TileType } from "pmtiles";
import { ContextMenu, type ContextMenuItem } from "@window/ui";
import { convertFileSrc } from "@window/bridge";
import type { FeatureCollection } from "geojson";
import type { LoadedSource } from "../types";

const props = defineProps<{ source: LoadedSource }>();
const emit = defineEmits<{
  error: [message: string];
  layers: [names: string[]];
}>();

const mapContainer = ref<HTMLDivElement | null>(null);
// Held outside Vue reactivity on purpose: MapLibre manages its own internal
// state and should not be wrapped in a reactive proxy.
let mapInstance: maplibregl.Map | null = null;

const SOURCE_IDS = ["geojson-data", "pmtiles"];
let addedLayerIds: string[] = [];

// The pmtiles:// protocol is registered once, process-wide.
let pmtilesProtocol: Protocol | null = null;
function ensurePmtilesProtocol(): Protocol {
  if (!pmtilesProtocol) {
    pmtilesProtocol = new Protocol();
    maplibregl.addProtocol("pmtiles", pmtilesProtocol.tile);
  }
  return pmtilesProtocol;
}

// Distinct colors so stacked vector layers stay legible over the base map.
const PALETTE = [
  "#2563eb",
  "#dc2626",
  "#16a34a",
  "#9333ea",
  "#ea580c",
  "#0891b2",
  "#ca8a04",
  "#db2777",
  "#4f46e5",
  "#65a30d",
];

// Key-free OpenFreeMap vector basemaps. These are full style documents (not a
// single base layer), so we switch with setStyle rather than toggling layer
// visibility.
const BASEMAPS = {
  positron: "https://tiles.openfreemap.org/styles/positron",
  bright: "https://tiles.openfreemap.org/styles/bright",
} as const;
type Basemap = keyof typeof BASEMAPS;
const basemap = ref<Basemap>("bright");

// Swap the base style while keeping the data overlay. transformStyle merges our
// sources/layers (identified by id) from the outgoing style into the incoming
// one; layers are appended last so they stay on top of the new base map.
function setBasemap(name: Basemap) {
  const map = mapInstance;
  if (!map || name === basemap.value) return;
  basemap.value = name;
  map.setStyle(BASEMAPS[name], {
    transformStyle: (prev, next) => {
      if (!prev) return next;
      const sources = { ...next.sources };
      for (const id of SOURCE_IDS) if (prev.sources[id]) sources[id] = prev.sources[id];
      const keep = new Set(addedLayerIds);
      return { ...next, sources, layers: [...next.layers, ...prev.layers.filter((l) => keep.has(l.id))] };
    },
  });
}

// ---- right-click basemap menu --------------------------------------------
const menuOpen = ref(false);
const menuX = ref(0);
const menuY = ref(0);

const menuItems = ref<ContextMenuItem[]>([]);
function openMenu(e: MouseEvent) {
  menuX.value = e.clientX;
  menuY.value = e.clientY;
  menuItems.value = [
    { id: "positron", label: "Positron (light)", checked: basemap.value === "positron" },
    { id: "bright", label: "Bright", checked: basemap.value === "bright" },
  ];
  menuOpen.value = true;
}
function onMenuSelect(id: string) {
  setBasemap(id as Basemap);
}

function escapeHtml(value: unknown): string {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function propertiesHtml(feature: MapGeoJSONFeature): string {
  const props = feature.properties || {};
  const keys = Object.keys(props);
  if (keys.length === 0) {
    return `<div class="feature-popup"><span class="empty">No properties</span></div>`;
  }
  const rows = keys
    .map((k) => `<tr><th>${escapeHtml(k)}</th><td>${escapeHtml(props[k])}</td></tr>`)
    .join("");
  return `<div class="feature-popup"><table>${rows}</table></div>`;
}

// Walk every coordinate in the collection to compute a bounding box.
function computeBounds(fc: FeatureCollection): maplibregl.LngLatBounds | null {
  const bounds = new maplibregl.LngLatBounds();
  let has = false;

  const walk = (coords: any) => {
    if (typeof coords[0] === "number") {
      bounds.extend([coords[0], coords[1]] as [number, number]);
      has = true;
    } else {
      for (const c of coords) walk(c);
    }
  };

  for (const feature of fc.features) {
    const geom = feature.geometry;
    if (!geom) continue;
    if (geom.type === "GeometryCollection") {
      for (const g of geom.geometries) {
        if ("coordinates" in g) walk(g.coordinates);
      }
    } else if ("coordinates" in geom) {
      walk(geom.coordinates);
    }
  }

  return has ? bounds : null;
}

// Remove every source/layer we added, leaving the OSM base intact.
function clearAdded(map: maplibregl.Map) {
  for (const id of addedLayerIds) if (map.getLayer(id)) map.removeLayer(id);
  for (const id of SOURCE_IDS) if (map.getSource(id)) map.removeSource(id);
  addedLayerIds = [];
}

interface GeometryStyle {
  fillColor: string;
  fillOpacity: number;
  lineColor: string;
  lineWidth: number;
  circleColor: string;
  circleRadius: number;
  circleOpacity: number;
  circleStroke?: boolean;
}

function addGeometryLayers(
  map: maplibregl.Map,
  opts: { idPrefix: string; source: string; sourceLayer?: string },
  style: GeometryStyle,
) {
  const base = opts.sourceLayer
    ? { source: opts.source, "source-layer": opts.sourceLayer }
    : { source: opts.source };
  const fill = `${opts.idPrefix}-fill`;
  const line = `${opts.idPrefix}-line`;
  const circle = `${opts.idPrefix}-circle`;

  map.addLayer({
    id: fill,
    type: "fill",
    ...base,
    filter: ["==", "$type", "Polygon"],
    paint: { "fill-color": style.fillColor, "fill-opacity": style.fillOpacity },
  });
  map.addLayer({
    id: line,
    type: "line",
    ...base,
    filter: ["any", ["==", "$type", "LineString"], ["==", "$type", "Polygon"]],
    paint: { "line-color": style.lineColor, "line-width": style.lineWidth },
  });
  map.addLayer({
    id: circle,
    type: "circle",
    ...base,
    filter: ["==", "$type", "Point"],
    paint: {
      "circle-radius": style.circleRadius,
      "circle-color": style.circleColor,
      "circle-opacity": style.circleOpacity,
      ...(style.circleStroke ? { "circle-stroke-color": "#ffffff", "circle-stroke-width": 1 } : {}),
    },
  });

  for (const id of [fill, line, circle]) {
    addedLayerIds.push(id);
    map.on("mouseenter", id, () => (map.getCanvas().style.cursor = "pointer"));
    map.on("mouseleave", id, () => (map.getCanvas().style.cursor = ""));
  }
}

function renderGeoJson(map: maplibregl.Map, fc: FeatureCollection) {
  map.addSource("geojson-data", { type: "geojson", data: fc });
  addGeometryLayers(
    map,
    { idPrefix: "gj", source: "geojson-data" },
    {
      fillColor: "#f97316",
      fillOpacity: 0.35,
      lineColor: "#ea580c",
      lineWidth: 2,
      circleColor: "#2563eb",
      circleRadius: 5,
      circleOpacity: 0.8,
      circleStroke: true,
    },
  );

  const bounds = computeBounds(fc);
  if (bounds) map.fitBounds(bounds, { padding: 40, maxZoom: 16, duration: 0 });
}

async function renderPmtiles(map: maplibregl.Map, path: string) {
  const url = convertFileSrc(path);
  const pmUrl = `pmtiles://${url}`;

  const protocol = ensurePmtilesProtocol();
  const archive = new PMTiles(url);
  protocol.add(archive);

  const header = await archive.getHeader();
  const isVector = header.tileType === TileType.Mvt || header.tileType === TileType.Mlt;

  if (isVector) {
    map.addSource("pmtiles", { type: "vector", url: pmUrl });

    const metadata = (await archive.getMetadata()) as {
      vector_layers?: Array<{ id: string }>;
    };
    const vectorLayers = metadata.vector_layers ?? [];
    if (vectorLayers.length === 0) {
      throw new Error(
        "This vector PMTiles archive has no 'vector_layers' metadata, so its layers can't be styled.",
      );
    }
    vectorLayers.forEach((vl, i) => {
      const color = PALETTE[i % PALETTE.length];
      addGeometryLayers(
        map,
        { idPrefix: `pm-${vl.id}`, source: "pmtiles", sourceLayer: vl.id },
        {
          fillColor: color,
          fillOpacity: 0.25,
          lineColor: color,
          lineWidth: 1.2,
          circleColor: color,
          circleRadius: 3,
          circleOpacity: 0.85,
        },
      );
    });
    emit(
      "layers",
      vectorLayers.map((vl) => vl.id),
    );
  } else {
    map.addSource("pmtiles", { type: "raster", url: pmUrl, tileSize: 256 });
    map.addLayer({
      id: "pmtiles-raster",
      type: "raster",
      source: "pmtiles",
      paint: { "raster-opacity": 0.85 },
    });
    addedLayerIds.push("pmtiles-raster");
    emit("layers", []);
  }

  // Fit to the archive's own bounds (skip if they're degenerate/unset).
  const { minLon, minLat, maxLon, maxLat } = header;
  if (minLon !== maxLon || minLat !== maxLat) {
    map.fitBounds(
      [
        [minLon, minLat],
        [maxLon, maxLat],
      ],
      { padding: 40, maxZoom: 16, duration: 0 },
    );
  }
}

async function renderSource(map: maplibregl.Map, source: LoadedSource) {
  clearAdded(map);
  try {
    if (source.kind === "geojson") {
      renderGeoJson(map, source.data);
    } else {
      await renderPmtiles(map, source.path);
    }
  } catch (e) {
    clearAdded(map);
    emit("error", e instanceof Error ? e.message : String(e));
  }
}

onMounted(() => {
  if (!mapContainer.value) return;

  const map = new maplibregl.Map({
    container: mapContainer.value,
    style: BASEMAPS[basemap.value],
    center: [-2.5, 54.5], // UK-centered default before data loads
    zoom: 5,
  });
  mapInstance = map;

  const popup = new maplibregl.Popup({ closeButton: true, maxWidth: "300px" });

  map.on("click", (e) => {
    if (addedLayerIds.length === 0) return;
    const features = map.queryRenderedFeatures(e.point, {
      layers: addedLayerIds,
    });
    if (!features.length) return;
    popup.setLngLat(e.lngLat).setHTML(propertiesHtml(features[0])).addTo(map);
  });

  map.on("load", () => renderSource(map, props.source));
});

// Re-render when a different file is opened while the map is already mounted.
watch(
  () => props.source,
  (source) => {
    const map = mapInstance;
    if (map && map.isStyleLoaded()) renderSource(map, source);
  },
);

onUnmounted(() => {
  mapInstance?.remove();
  mapInstance = null;
});
</script>

<template>
  <div ref="mapContainer" class="map-container" @contextmenu.prevent="openMenu"></div>
  <ContextMenu
    :open="menuOpen"
    :x="menuX"
    :y="menuY"
    :items="menuItems"
    @select="onMenuSelect"
    @close="menuOpen = false"
  />
</template>

<style scoped>
.map-container {
  flex: 1;
  min-height: 0;
  width: 100%;
}
</style>
