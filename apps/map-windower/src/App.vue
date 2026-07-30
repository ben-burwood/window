<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import MapView from "./components/MapView.vue";
import { toFeatureCollection, fileKind, type LoadedSource } from "./types";

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
      const command = kind === "geoparquet" ? "load_geoparquet" : "load_file";
      const text = await invoke<string>(command, { path });
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

async function openFile() {
  const path = (await open({
    multiple: false,
    filters: [
      { name: "Map data", extensions: ["geojson", "pmtiles", "geoparquet"] },
    ],
  })) as string | null;
  if (!path) return;
  await loadFileByPath(path);
}

onMounted(async () => {
  const startupFile = await invoke<string | null>("get_startup_file");
  if (startupFile) await loadFileByPath(startupFile);
});
</script>

<template>
  <div class="app">
    <header v-if="loaded" class="topbar">
      <span class="file-name">{{ loaded.name }}</span>
      <span v-if="layerNames.length" class="layer-names">{{ layerNames.join(", ") }}</span>
      <button class="open-btn" @click="openFile">Open File</button>
    </header>

    <main class="content">
      <MapView
        v-if="loaded"
        :source="loaded"
        @error="onMapError"
        @layers="onLayers"
      />

      <div v-else class="overlay">
        <div class="drop-zone">
          <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round"
              d="M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z" />
          </svg>
          <p v-if="error" class="hint error">{{ error }}</p>
          <p v-else class="hint">Open a GeoJSON, PMTiles or GeoParquet file to get started</p>
          <button @click="openFile" :disabled="loading">
            {{ loading ? "Loading…" : "Open File" }}
          </button>
        </div>
      </div>
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

.topbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 14px;
  background: #f8f8f8;
  border-bottom: 1px solid #babfc7;
  font-size: 14px;
  flex-shrink: 0;
}

.file-name {
  color: #475569;
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.layer-names {
  color: #888;
  font-size: 0.8rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.open-btn {
  margin-left: auto;
  padding: 4px 14px;
  font-size: 0.8rem;
}

.content {
  flex: 1;
  min-height: 0;
  display: flex;
}

/* Launch screen — matches data-framer's empty state */
.overlay {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.drop-zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 48px 64px;
  border: 2px dashed #babfc7;
  border-radius: 16px;
  color: #181d1f;
}

.icon {
  width: 48px;
  height: 48px;
  color: #aaa;
}

.hint {
  margin: 0;
  font-size: 0.95rem;
  color: #888;
}

.hint.error {
  color: #b91c1c;
  max-width: 360px;
  text-align: center;
}

button {
  padding: 10px 28px;
  font-size: 14px;
  font-family: inherit;
  border-radius: 4px;
  border: none;
  background: #2196f3;
  color: #fff;
  cursor: pointer;
  transition: background 0.15s;
}

button:hover:not(:disabled) {
  background: color-mix(in srgb, #2196f3 82%, #000);
}

button:disabled {
  opacity: 0.55;
  cursor: default;
}
</style>
