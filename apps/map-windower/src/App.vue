<script setup lang="ts">
import { onMounted, ref } from "vue";
import { EmptyState, Toolbar, ToolbarButton } from "@viewers/ui";
import MapView from "./components/MapView.vue";
import { toFeatureCollection, fileKind, type LoadedSource } from "./types";
import { getStartupFile, openFile, loadGeojsonText, loadGeoparquetText } from "./bridge";

const loaded = ref<LoadedSource | null>(null);
const layerNames = ref<string[]>([]);
const error = ref<string | null>(null);
const loading = ref(false);

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

async function loadFileByPath(path: string) {
  loading.value = true;
  error.value = null;
  layerNames.value = [];
  try {
    const name = basename(path);
    const kind = fileKind(path);
    if (kind === "pmtiles") {
      // The map reads the archive directly via the pmtiles:// protocol.
      loaded.value = { kind: "pmtiles", name, path };
    } else {
      const text = kind === "geoparquet" ? await loadGeoparquetText(path) : await loadGeojsonText(path);
      const data = toFeatureCollection(JSON.parse(text));
      loaded.value = { kind: "geojson", name, data };
    }
  } catch (e) {
    loaded.value = null;
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

function onMapError(message: string) {
  loaded.value = null;
  error.value = message;
}

function onLayers(names: string[]) {
  layerNames.value = names;
}

async function chooseFile() {
  const path = await openFile([
    { name: "Map data", extensions: ["geojson", "pmtiles", "geoparquet"] },
  ]);
  if (path) await loadFileByPath(path);
}

onMounted(async () => {
  const startupFile = await getStartupFile();
  if (startupFile) await loadFileByPath(startupFile);
});
</script>

<template>
  <div class="app">
    <Toolbar v-if="loaded">
      <template #start>
        <span class="file-name">{{ loaded.name }}</span>
        <span v-if="layerNames.length" class="layer-names">{{ layerNames.join(", ") }}</span>
      </template>
      <template #end>
        <ToolbarButton variant="primary" @click="chooseFile">Open File</ToolbarButton>
      </template>
    </Toolbar>

    <main class="content">
      <MapView v-if="loaded" :source="loaded" @error="onMapError" @layers="onLayers" />

      <EmptyState
        v-else
        title="Open a map file"
        hint="Open a GeoJSON, PMTiles or GeoParquet file to get started"
        :error="error"
        :action-label="loading ? 'Loading…' : 'Open File'"
        @open="chooseFile"
      />
    </main>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
}

.file-name {
  color: var(--vw-fg);
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.layer-names {
  color: var(--vw-fg-muted);
  font-size: var(--vw-fs-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.content {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
}
</style>
