<script setup lang="ts">
import { ref, computed, defineAsyncComponent, onMounted } from "vue";
import type { ColumnState, GridApi } from "ag-grid-community";
import { EmptyState, Toolbar, ToolbarButton } from "@viewers/ui";
import type { FileInfo, FilterSpec } from "./types";
import { getStartupFile, openFile, saveFile, loadFile, exportFile as exportFileCmd } from "./bridge";
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
const error = ref<string | null>(null);

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
  error.value = null;
  gridApi.value = null;
  filterPanelOpen.value = false;
  activeFilters.value = [];
  filteredRowCount.value = 0;
  columnPanelOpen.value = false;

  try {
    const info = await loadFile(path);
    fileInfo.value = info;
    initColumnVisibility();
    currentView.value = "table";
    view.value = "loaded";
  } catch (err) {
    view.value = "empty";
    error.value = err instanceof Error ? err.message : String(err);
  }
}

async function chooseFile() {
  const path = await openFile([{ name: "Data Files", extensions: ["parquet", "csv"] }]);
  if (!path) return;
  await loadFileByPath(path);
}

onMounted(async () => {
  const startupFile = await getStartupFile();
  if (startupFile) await loadFileByPath(startupFile);
});

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------
async function exportFile() {
  const dest = await saveFile([
    { name: "CSV",     extensions: ["csv"]     },
    { name: "Parquet", extensions: ["parquet"] },
  ]);
  if (!dest) return;

  const sortedCol = (gridApi.value?.getColumnState() as ColumnState[] ?? [])
    .find(c => c.sort != null);

  const allCols = fileInfo.value!.columns;
  const visible = allCols.filter(c => activeColumnVisibility.value[c.name] !== false);
  const columns = visible.length < allCols.length ? visible.map(c => c.name) : [];

  exportState.value = "exporting";
  try {
    await exportFileCmd({
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
    <Toolbar v-if="view === 'loaded'">
      <template #start>
        <span class="file-name">{{ fileName }}</span>
        <span class="row-count">
          <template v-if="activeFilters.length > 0">
            {{ filteredRowCount.toLocaleString() }} / {{ fileInfo!.total_rows.toLocaleString() }} rows
          </template>
          <template v-else>
            {{ fileInfo!.total_rows.toLocaleString() }} rows
          </template>
        </span>
      </template>

      <template #center>
        <ToolbarButton :active="filterPanelOpen" @click="filterPanelOpen = !filterPanelOpen">
          Filters<span v-if="activeFilters.length > 0" class="badge">{{ activeFilters.length }}</span>
        </ToolbarButton>
        <ToolbarButton :active="columnPanelOpen" @click="columnPanelOpen = !columnPanelOpen">
          Columns<span v-if="hiddenColumnCount > 0" class="badge">{{ hiddenColumnCount }}</span>
        </ToolbarButton>
        <div class="view-toggle">
          <ToolbarButton :active="currentView === 'table'" @click="currentView = 'table'">Table</ToolbarButton>
          <ToolbarButton v-if="hasMapData" :active="currentView === 'map'" @click="currentView = 'map'">Map</ToolbarButton>
          <ToolbarButton :active="currentView === 'chart'" @click="currentView = 'chart'">Chart</ToolbarButton>
        </div>
      </template>

      <template #end>
        <ToolbarButton :disabled="exportState === 'exporting'" @click="exportFile">
          {{ exportState === "exporting" ? "Exporting…" : "Export" }}
        </ToolbarButton>
        <ToolbarButton variant="primary" @click="chooseFile">Open File</ToolbarButton>
      </template>
    </Toolbar>

    <main class="content">
      <template v-if="view === 'loaded'">
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

      <EmptyState
        v-else
        title="Open a dataset"
        hint="Open a Parquet or CSV file to get started"
        :error="error"
        :action-label="view === 'loading' ? 'Loading…' : 'Open File'"
        @open="chooseFile"
      />
    </main>
  </div>
</template>

<style scoped>
.app {
  /* AG Grid Quartz design tokens — driven by the shared @viewers/ui --vw-* tokens so the grid,
     toolbar, panels and shared.css all follow one theme. */
  --ag-active-color: var(--vw-accent);
  --ag-background-color: var(--vw-bg);
  --ag-foreground-color: var(--vw-fg);
  --ag-border-color: var(--vw-border);
  --ag-header-background-color: var(--vw-surface);
  --ag-row-hover-color: color-mix(in srgb, transparent, var(--ag-active-color) 12%);
  --ag-disabled-foreground-color: var(--vw-fg-muted);
  --ag-input-border-color: var(--ag-border-color);
  --ag-input-focus-border-color: var(--ag-active-color);
  --ag-input-focus-box-shadow: 0 0 0 3px color-mix(in srgb, transparent, var(--ag-active-color) 47%);
  --ag-chip-background-color: var(--vw-surface-2);
  --ag-font-family: var(--vw-font);
  --ag-font-size: 14px;
  --ag-border-radius: var(--vw-radius-sm);

  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--vw-bg);
}

.content {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.file-name {
  font-weight: 600;
  color: var(--vw-fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-count {
  color: var(--vw-fg-muted);
  font-size: var(--vw-fs-sm);
  white-space: nowrap;
}

.view-toggle {
  display: flex;
  align-items: center;
  gap: var(--vw-space-1);
  flex-shrink: 0;
}

.badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  margin-left: var(--vw-space-1);
  background: var(--vw-accent);
  color: var(--vw-accent-fg);
  font-size: var(--vw-fs-xs);
  font-weight: 700;
  min-width: 16px;
  height: 16px;
  border-radius: 8px;
  padding: 0 4px;
}
</style>
