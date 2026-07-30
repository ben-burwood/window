<script setup lang="ts">
import { ref, computed, defineAsyncComponent, onMounted } from "vue";
import type { ColumnState, GridApi } from "ag-grid-community";
import { open, save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type { FileInfo, FilterSpec } from "./types";
import FilterPanel from "./components/FilterPanel.vue";
import SelectPanel from "./components/SelectPanel.vue";
import DataGrid from "./components/DataGrid.vue";
const MapView   = defineAsyncComponent(() => import("./components/MapView.vue"));
const ChartView = defineAsyncComponent(() => import("./components/ChartView.vue"));

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
type ViewState = "empty" | "loading" | "loaded";

const view = ref<ViewState>("empty");
const fileInfo = ref<FileInfo | null>(null);
const activeFilters = ref<FilterSpec[]>([]);
const activeColumnVisibility = ref<Record<string, boolean>>({});
const currentView = ref<"table" | "map" | "chart">("table");
const exportState = ref<"idle" | "exporting">("idle");
const filteredRowCount = ref(0);
const gridApi = ref<GridApi | null>(null);
const filterPanelOpen = ref(false);
const columnPanelOpen = ref(false);

// ---------------------------------------------------------------------------
// Computed
// ---------------------------------------------------------------------------
const LAT_NAMES = ["lat", "latitude"];
const LON_NAMES = ["lon", "lng", "longitude"];
const H3_NAMES  = ["h3", "h3_index", "h3index", "h3_cell", "h3cell", "h3point"];
const GEOM_NAMES = ["geometry", "geom", "the_geom", "wkb_geometry", "wkb"];

const fileName = computed(() => fileInfo.value?.path.split(/[\\/]/).pop() ?? "");

const latColumn = computed(() =>
  fileInfo.value?.columns.find(c => LAT_NAMES.includes(c.name.toLowerCase()))?.name ?? null
);
const lonColumn = computed(() =>
  fileInfo.value?.columns.find(c => LON_NAMES.includes(c.name.toLowerCase()))?.name ?? null
);
const h3Column = computed(() =>
  fileInfo.value?.columns.find(c => H3_NAMES.includes(c.name.toLowerCase()))?.name ?? null
);
const geomColumn = computed(() =>
  fileInfo.value?.columns.find(
    c => c.dtype === "binary" && GEOM_NAMES.includes(c.name.toLowerCase())
  )?.name ?? null
);
const hasMapData = computed(() =>
  (!!latColumn.value && !!lonColumn.value) || !!h3Column.value || !!geomColumn.value
);

// Auto-detect the best default x-axis column for charts:
// prefer datetime > date > numeric > first column
const defaultXColumn = computed(() => {
  const cols = fileInfo.value?.columns ?? [];
  return (
    cols.find(c => c.dtype === "datetime")?.name ??
    cols.find(c => c.dtype === "date")?.name ??
    cols.find(c => c.dtype === "integer" || c.dtype === "float" || c.dtype === "decimal")?.name ??
    cols[0]?.name ??
    null
  );
});

const hiddenColumnCount = computed(() => {
  if (!fileInfo.value) return 0;
  return fileInfo.value.columns.filter(c => activeColumnVisibility.value[c.name] === false).length;
});

// ---------------------------------------------------------------------------
// File open
// ---------------------------------------------------------------------------
function initColumnVisibility() {
  const vis: Record<string, boolean> = {};
  for (const c of fileInfo.value!.columns) vis[c.name] = true;
  activeColumnVisibility.value = vis;
}

async function loadFileByPath(path: string) {
  view.value = "loading";
  gridApi.value = null;
  filterPanelOpen.value = false;
  activeFilters.value = [];
  filteredRowCount.value = 0;
  columnPanelOpen.value = false;

  try {
    const info = await invoke<FileInfo>("load_file", { path });
    fileInfo.value = info;
    initColumnVisibility();
    currentView.value = "table";
    view.value = "loaded";
  } catch (err) {
    view.value = "empty";
    alert(`Failed to load file:\n${err}`);
  }
}

async function openFile() {
  const path = (await open({
    multiple: false,
    filters: [{ name: "Data Files", extensions: ["parquet", "csv"] }],
  })) as string | null;
  if (!path) return;
  await loadFileByPath(path);
}

onMounted(async () => {
  const startupFile = await invoke<string | null>("get_startup_file");
  if (startupFile) await loadFileByPath(startupFile);
});

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------
async function exportFile() {
  const dest = (await save({
    filters: [
      { name: "CSV",     extensions: ["csv"]     },
      { name: "Parquet", extensions: ["parquet"] },
    ],
  })) as string | null;
  if (!dest) return;

  const sortedCol = (gridApi.value?.getColumnState() as ColumnState[] ?? [])
    .find(c => c.sort != null);

  const allCols = fileInfo.value!.columns;
  const visible = allCols.filter(c => activeColumnVisibility.value[c.name] !== false);
  const columns = visible.length < allCols.length ? visible.map(c => c.name) : [];

  exportState.value = "exporting";
  try {
    await invoke("export_file", {
      dest,
      sortCol:  sortedCol?.colId  ?? null,
      sortDesc: sortedCol?.sort === "desc",
      filters:  activeFilters.value,
      columns,
    });
  } catch (err) {
    alert(`Export failed:\n${err}`);
  } finally {
    exportState.value = "idle";
  }
}

// ---------------------------------------------------------------------------
// Filter / column event handlers
// ---------------------------------------------------------------------------
function onFiltersApply(filters: FilterSpec[]) {
  activeFilters.value = filters;
}

function onFiltersClear() {
  activeFilters.value = [];
  filteredRowCount.value = 0;
}

function onColumnsReset() {
  initColumnVisibility();
}
</script>

<template>
  <div class="app">
    <!-- Empty / loading overlay -->
    <div v-if="view !== 'loaded'" class="overlay">
      <div class="drop-zone">
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round"
            d="M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z" />
        </svg>
        <p class="hint">Open a Parquet or CSV file to get started</p>
        <button @click="openFile" :disabled="view === 'loading'">
          {{ view === "loading" ? "Loading…" : "Open File" }}
        </button>
      </div>
    </div>

    <!-- Loaded state -->
    <template v-else>
      <div class="toolbar">
        <span class="file-name">{{ fileName }}</span>
        <span class="row-count">
          <template v-if="activeFilters.length > 0">
            {{ filteredRowCount.toLocaleString() }} / {{ fileInfo!.total_rows.toLocaleString() }} rows
          </template>
          <template v-else>
            {{ fileInfo!.total_rows.toLocaleString() }} rows
          </template>
        </span>
        <button
          class="toolbar-btn filters-btn"
          :class="{ active: filterPanelOpen }"
          @click="filterPanelOpen = !filterPanelOpen"
        >
          Filters<span v-if="activeFilters.length > 0" class="badge">{{ activeFilters.length }}</span>
        </button>
        <button
          class="toolbar-btn columns-btn"
          :class="{ active: columnPanelOpen }"
          @click="columnPanelOpen = !columnPanelOpen"
        >
          Columns<span v-if="hiddenColumnCount > 0" class="badge">{{ hiddenColumnCount }}</span>
        </button>
        <div class="view-toggle">
          <button :class="{ active: currentView === 'table' }" @click="currentView = 'table'">Table</button>
          <button v-if="hasMapData" :class="{ active: currentView === 'map' }" @click="currentView = 'map'">Map</button>
          <button :class="{ active: currentView === 'chart' }" @click="currentView = 'chart'">Chart</button>
        </div>
        <button
          class="toolbar-btn"
          :disabled="exportState === 'exporting'"
          @click="exportFile"
        >{{ exportState === "exporting" ? "Exporting…" : "Export" }}</button>
        <button class="toolbar-btn" @click="openFile">Open File</button>
      </div>

      <FilterPanel
        v-show="filterPanelOpen"
        :columns="fileInfo!.columns"
        @apply="onFiltersApply"
        @clear="onFiltersClear"
      />
      <SelectPanel
        v-show="columnPanelOpen"
        :columns="fileInfo!.columns"
        :activeColumnVisibility="activeColumnVisibility"
        @apply="activeColumnVisibility = $event"
        @reset="onColumnsReset"
      />
      <DataGrid
        v-show="currentView === 'table'"
        :columns="fileInfo!.columns"
        :activeFilters="activeFilters"
        :activeColumnVisibility="activeColumnVisibility"
        @ready="gridApi = $event"
        @row-count-changed="filteredRowCount = $event"
      />
      <MapView
        v-if="hasMapData"
        v-show="currentView === 'map'"
        :active="currentView === 'map'"
        :activeFilters="activeFilters"
        :latColumn="latColumn"
        :lonColumn="lonColumn"
        :h3Column="h3Column"
        :geomColumn="geomColumn"
      />
      <ChartView
        v-show="currentView === 'chart'"
        :columns="fileInfo!.columns"
        :activeFilters="activeFilters"
        :defaultXColumn="defaultXColumn"
      />
    </template>
  </div>
</template>

<style>
*,
*::before,
*::after {
  box-sizing: border-box;
}

html,
body,
#app {
  height: 100%;
  margin: 0;
  padding: 0;
  overflow: hidden;
  /* IBM Plex Sans matches the AG Grid Quartz font stack */
  font-family: "IBM Plex Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
}
</style>

<style scoped>
.app {
  /* AG Grid Quartz design tokens — used by toolbar, panels, and shared.css */
  --ag-active-color: #2196f3;
  --ag-background-color: #fff;
  --ag-foreground-color: #181d1f;
  --ag-border-color: color-mix(in srgb, transparent, var(--ag-foreground-color) 15%);
  --ag-header-background-color: color-mix(in srgb, var(--ag-background-color), var(--ag-foreground-color) 2%);
  --ag-row-hover-color: color-mix(in srgb, transparent, var(--ag-active-color) 12%);
  --ag-disabled-foreground-color: color-mix(in srgb, transparent, var(--ag-foreground-color) 50%);
  --ag-input-border-color: var(--ag-border-color);
  --ag-input-focus-border-color: var(--ag-active-color);
  --ag-input-focus-box-shadow: 0 0 0 3px color-mix(in srgb, transparent, var(--ag-active-color) 47%);
  --ag-chip-background-color: color-mix(in srgb, transparent, var(--ag-foreground-color) 7%);
  --ag-font-family: "IBM Plex Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --ag-font-size: 14px;
  --ag-border-radius: 4px;

  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--ag-background-color);
}

/* ---- Empty / loading ---- */
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
  border: 2px dashed var(--ag-border-color, #babfc7);
  border-radius: 16px;
  color: var(--ag-foreground-color, #181d1f);
}

.icon {
  width: 48px;
  height: 48px;
  color: var(--ag-disabled-foreground-color, #aaa);
}

.hint {
  margin: 0;
  font-size: 0.95rem;
  color: var(--ag-disabled-foreground-color, #888);
}

button {
  padding: 10px 28px;
  font-size: var(--ag-font-size, 14px);
  font-family: var(--ag-font-family, inherit);
  border-radius: var(--ag-border-radius, 4px);
  border: none;
  background: var(--ag-active-color, #2196f3);
  color: #fff;
  cursor: pointer;
  transition: background 0.15s;
}

button:hover:not(:disabled) {
  background: color-mix(in srgb, var(--ag-active-color, #2196f3) 82%, #000);
}

button:disabled {
  opacity: 0.55;
  cursor: default;
}

/* ---- Toolbar ---- */
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 14px;
  background: var(--ag-header-background-color, #f8f8f8);
  border-bottom: 1px solid var(--ag-border-color, #babfc7);
  font-size: var(--ag-font-size, 14px);
  font-family: var(--ag-font-family, inherit);
  flex-shrink: 0;
}

.file-name {
  font-weight: 600;
  color: var(--ag-foreground-color, #181d1f);
}

.row-count {
  color: var(--ag-disabled-foreground-color, #777);
}

.toolbar-btn {
  padding: 4px 14px;
  font-size: 0.8rem;
}

.toolbar-btn:last-child {
  margin-left: auto;
}

.filters-btn,
.columns-btn {
  background: transparent;
  color: var(--ag-foreground-color, #444);
  border: 1px solid var(--ag-border-color, #babfc7);
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.filters-btn:hover:not(:disabled),
.columns-btn:hover:not(:disabled) {
  background: var(--ag-row-hover-color, rgba(33, 150, 243, 0.12));
}

.filters-btn.active,
.columns-btn.active {
  background: color-mix(in srgb, transparent, var(--ag-active-color, #2196f3) 10%);
  color: var(--ag-active-color, #2196f3);
  border-color: var(--ag-active-color, #2196f3);
}

.badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--ag-active-color, #2196f3);
  color: #fff;
  font-size: 0.7rem;
  font-weight: 700;
  min-width: 16px;
  height: 16px;
  border-radius: 8px;
  padding: 0 4px;
}

/* ---- View toggle ---- */
.view-toggle {
  display: flex;
  flex-shrink: 0;
}

.view-toggle button {
  padding: 4px 12px;
  font-size: 0.8rem;
  background: var(--ag-background-color, #fff);
  color: var(--ag-foreground-color, #444);
  border: 1px solid var(--ag-border-color, #babfc7);
  border-radius: 0;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.view-toggle button:first-child {
  border-radius: var(--ag-border-radius, 4px) 0 0 var(--ag-border-radius, 4px);
}

.view-toggle button:last-child {
  border-radius: 0 var(--ag-border-radius, 4px) var(--ag-border-radius, 4px) 0;
}

.view-toggle button:not(:first-child) {
  border-left: none;
}

.view-toggle button:hover:not(.active) {
  background: var(--ag-row-hover-color, rgba(33, 150, 243, 0.12));
}

.view-toggle button.active {
  background: var(--ag-active-color, #2196f3);
  color: #fff;
  border-color: var(--ag-active-color, #2196f3);
}
</style>
