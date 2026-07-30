<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { AgGridVue } from "ag-grid-vue3";
import {
  ModuleRegistry,
  themeQuartz,
  InfiniteRowModelModule,
  ColumnAutoSizeModule,
  ColumnApiModule,
  CellStyleModule,
  ValidationModule,
} from "ag-grid-community";
import type {
  CellContextMenuEvent,
  ColDef,
  FirstDataRenderedEvent,
  GridApi,
  GridReadyEvent,
  IDatasource,
  IGetRowsParams,
  ValueFormatterParams,
} from "ag-grid-community";
import { invoke } from "@tauri-apps/api/core";
import { formatCellValue } from "../types";
import type { ColumnInfo, FilterSpec, RowsResponse } from "../types";

ModuleRegistry.registerModules([
  InfiniteRowModelModule,
  ColumnAutoSizeModule,
  ColumnApiModule,
  CellStyleModule,
  ValidationModule,
]);

const gridTheme = themeQuartz.withParams({
  headerBackgroundColor: "color-mix(in srgb, white, #2196f3 12%)",
});

const props = defineProps<{
  columns: ColumnInfo[];
  activeFilters: FilterSpec[];
  activeColumnVisibility: Record<string, boolean>;
}>();

const emit = defineEmits<{
  ready: [api: GridApi];
  "row-count-changed": [count: number];
}>();

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
const gridApi = ref<GridApi | null>(null);
const pendingFetches = ref(0);
const isFetching = computed(() => pendingFetches.value > 0);

// ---------------------------------------------------------------------------
// Column defs
// ---------------------------------------------------------------------------
const visibleColumns = computed(() =>
  props.columns.filter(c => props.activeColumnVisibility[c.name] !== false)
);

// Nested cells (list/struct) arrive from the backend as real JSON arrays/objects.
// AG Grid's default renderer would stringify them to "[object Object]", so render
// them as compact valid JSON instead.
function jsonValueFormatter(params: ValueFormatterParams): string {
  return formatCellValue(params.value);
}

const NUMERIC_DTYPES: readonly string[] = ["integer", "float", "decimal"];
const NESTED_DTYPES: readonly string[] = ["list", "struct"];

const columnDefs = computed<ColDef[]>(() =>
  visibleColumns.value.map((c) => ({
    field: c.name,
    headerName: c.name,
    ...(NUMERIC_DTYPES.includes(c.dtype) ? { type: "numericColumn" } : {}),
    ...(NESTED_DTYPES.includes(c.dtype) ? { valueFormatter: jsonValueFormatter } : {}),
  }))
);

const selectedColumnNames = computed(() => {
  const visible = visibleColumns.value.map(c => c.name);
  return visible.length < props.columns.length ? visible : [];
});

const defaultColDef: ColDef = {
  minWidth: 80,
  sortable: true,
  resizable: true,
};

// ---------------------------------------------------------------------------
// Datasource
// ---------------------------------------------------------------------------
function buildDatasource(): IDatasource {
  return {
    async getRows(params: IGetRowsParams) {
      const { startRow, endRow, sortModel } = params;
      pendingFetches.value++;
      try {
        const r = await invoke<RowsResponse>("get_rows", {
          offset: startRow,
          limit: endRow - startRow,
          sortCol: sortModel[0]?.colId ?? null,
          sortDesc: sortModel[0]?.sort === "desc",
          filters: props.activeFilters,
          columns: selectedColumnNames.value,
        });
        emit("row-count-changed", r.total_rows);
        params.successCallback(r.rows, r.total_rows);
      } catch (err) {
        console.error("get_rows failed:", err);
        params.failCallback();
      } finally {
        pendingFetches.value--;
      }
    },
  };
}

// ---------------------------------------------------------------------------
// Grid events
// ---------------------------------------------------------------------------
function onGridReady(event: GridReadyEvent) {
  gridApi.value = event.api;
  event.api.setGridOption("datasource", buildDatasource());
  emit("ready", event.api);
}

function onFirstDataRendered(event: FirstDataRenderedEvent) {
  event.api.autoSizeAllColumns();
}

// ---------------------------------------------------------------------------
// Copy cell on right-click
// ---------------------------------------------------------------------------
function onCellContextMenu(event: CellContextMenuEvent) {
  // Nested cells are objects/arrays — copy them as valid JSON, not "[object Object]".
  const text = formatCellValue(event.value);
  navigator.clipboard.writeText(text).catch(() => {
    // Clipboard access was denied or failed; silently ignore
  });
}

// ---------------------------------------------------------------------------
// React to prop changes
// ---------------------------------------------------------------------------
watch(
  [() => props.activeFilters, () => props.activeColumnVisibility],
  () => { gridApi.value?.purgeInfiniteCache(); },
  { deep: true },
);
</script>

<template>
  <div class="grid-wrapper" :class="{ fetching: isFetching }">
    <AgGridVue
      class="grid"
      :theme="gridTheme"
      :columnDefs="columnDefs"
      :defaultColDef="defaultColDef"
      rowModelType="infinite"
      :cacheBlockSize="200"
      :maxBlocksInCache="20"
      :infiniteInitialRowCount="1"
      @grid-ready="onGridReady"
      @first-data-rendered="onFirstDataRendered"
      @cell-context-menu="onCellContextMenu"
    />
  </div>
</template>

<style scoped>
.grid-wrapper {
  flex: 1;
  min-height: 0;
  position: relative;
}

.grid {
  width: 100%;
  height: 100%;
}
</style>
