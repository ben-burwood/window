<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  Badge,
  ContextMenu,
  EmptyState,
  Toolbar,
  ToolbarButton,
  useContextMenu,
  useTheme,
} from "@window/ui";
import MapView from "./components/MapView.vue";
import OverviewPanel from "./components/OverviewPanel.vue";
import { buildOverview, type OverviewNode } from "./overview";
import { toFeatureCollection, fileKind, type LoadedSource } from "./types";
import {
  getStartupFile,
  openFile,
  loadGeojsonText,
  loadGeoparquetText,
  onFileDrop,
  onOpenFile,
  watchFile,
  onFileChanged,
} from "./bridge";

const loaded = ref<LoadedSource | null>(null);
const layerNames = ref<string[]>([]);
const error = ref<string | null>(null);
const loading = ref(false);
const dragging = ref(false);
const outdated = ref(false);
const showOverview = ref(false);
const unlisteners: Array<() => void> = [];

const overviewAvailable = computed(() => loaded.value?.kind === "geojson");
const overviewRoot = computed<OverviewNode | null>(() =>
  loaded.value?.kind === "geojson" ? buildOverview(loaded.value.data) : null,
);

const mapView = ref<InstanceType<typeof MapView> | null>(null);
const { open: menuOpen, x: menuX, y: menuY, openMenu, close: closeMenu } = useContextMenu();
const { menuItem: themeItem, handleSelect: handleThemeSelect } = useTheme();
const menuItems = computed(() => {
  const base = mapView.value?.basemapItems() ?? [];
  return base.length
    ? [...base, { id: "sep", separator: true }, themeItem.value]
    : [themeItem.value];
});
function onMenuSelect(id: string) {
  if (handleThemeSelect(id)) return;
  mapView.value?.selectBasemap(id);
}

// Extensions this app can open (validated on drag-drop).
const SUPPORTED_EXTENSIONS = ["geojson", "pmtiles", "geoparquet"];
function isSupported(path: string): boolean {
  const lower = path.toLowerCase();
  return SUPPORTED_EXTENSIONS.some((ext) => lower.endsWith(`.${ext}`));
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

async function loadFileByPath(path: string, isRescan = false) {
  loading.value = true;
  if (!isRescan) {
    error.value = null;
    layerNames.value = [];
  }
  try {
    const name = basename(path);
    const kind = fileKind(path);
    if (kind === "pmtiles") {
      // The map reads the archive directly via the pmtiles:// protocol.
      loaded.value = { kind: "pmtiles", name, path };
    } else {
      const text =
        kind === "geoparquet" ? await loadGeoparquetText(path) : await loadGeojsonText(path);
      const data = toFeatureCollection(JSON.parse(text));
      loaded.value = { kind: "geojson", name, data };
    }
    error.value = null;
    await watchFile(path);
    outdated.value = false;
  } catch (e) {
    if (isRescan) {
      // Transient read during a rescan (e.g. editor mid-save): keep current data, flag stale.
      outdated.value = true;
    } else {
      loaded.value = null;
      error.value = e instanceof Error ? e.message : String(e);
    }
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

function reload() {
  if (lastOpened) loadFileByPath(lastOpened, true);
}

// Single deduped entry point: every source (dialog, startup, drop, onOpenFile)
// funnels through here so the same file never loads twice.
let lastOpened: string | null = null;
async function openPath(path: string) {
  if (path === lastOpened) return;
  lastOpened = path;
  await loadFileByPath(path);
}

function handleDrop(paths: string[]) {
  const path = paths.find(isSupported);
  if (path) openPath(path);
  else error.value = "Please drop a GeoJSON, PMTiles or GeoParquet file.";
}

async function chooseFile() {
  const path = await openFile([
    { name: "Map data", extensions: ["geojson", "pmtiles", "geoparquet"] },
  ]);
  if (path) await openPath(path);
}

onMounted(async () => {
  const un1 = await onFileDrop(handleDrop, (h) => (dragging.value = h));
  const un2 = await onOpenFile(openPath);
  const un3 = await onFileChanged(() => {
    outdated.value = true;
  });
  unlisteners.push(un1, un2, un3);

  const startup = await getStartupFile();
  if (startup) await openPath(startup);
});
onUnmounted(() => unlisteners.forEach((u) => u()));
</script>

<template>
  <div class="app" @contextmenu="openMenu">
    <Toolbar v-if="loaded">
      <template #start>
        <span class="file-name">{{ loaded.name }}</span>
        <Badge
          v-if="outdated"
          variant="warning"
          interactive
          title="File changed on disk — click to reload"
          @click="reload"
          >outdated</Badge
        >
        <span v-if="layerNames.length" class="layer-names">{{ layerNames.join(", ") }}</span>
      </template>
      <template #end>
        <ToolbarButton
          v-if="overviewAvailable"
          :active="showOverview"
          title="Toggle GeoJSON structure"
          @click="showOverview = !showOverview"
          >Overview</ToolbarButton
        >
        <ToolbarButton variant="primary" @click="chooseFile">Open File</ToolbarButton>
      </template>
    </Toolbar>

    <main class="content">
      <template v-if="loaded">
        <MapView ref="mapView" :source="loaded" @error="onMapError" @layers="onLayers" />

        <OverviewPanel
          v-if="showOverview && overviewRoot"
          :root="overviewRoot"
          @close="showOverview = false"
        />
      </template>

      <EmptyState
        v-else
        title="Open a map file"
        hint="Open a GeoJSON, PMTiles or GeoParquet file to get started"
        :error="error"
        :dragging="dragging"
        :action-label="loading ? 'Loading…' : 'Open File'"
        @open="chooseFile"
      />
    </main>

    <ContextMenu
      :open="menuOpen"
      :x="menuX"
      :y="menuY"
      :items="menuItems"
      @select="onMenuSelect"
      @close="closeMenu"
    />
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
