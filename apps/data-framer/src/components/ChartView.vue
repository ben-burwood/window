<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { AgCharts } from "ag-charts-vue3";
import type { AgChartOptions } from "ag-charts-community";
import type { ChartConfig, ColumnInfo, FilterSpec } from "../types";

const props = defineProps<{
  columns: ColumnInfo[];
  activeFilters: FilterSpec[];
  defaultXColumn: string | null;
}>();

// ---------------------------------------------------------------------------
// Derived column lists
// ---------------------------------------------------------------------------
const numericColumns = computed(() =>
  props.columns.filter(c => c.dtype === "integer" || c.dtype === "float" || c.dtype === "decimal")
);

// ---------------------------------------------------------------------------
// Pending (editable) config
// ---------------------------------------------------------------------------
type ChartType = "line" | "scatter" | "bar";
const chartTypes: { value: ChartType; label: string }[] = [
  { value: "line", label: "Line" },
  { value: "scatter", label: "Scatter" },
  { value: "bar", label: "Bar" },
];
const pendingChartType = ref<ChartType>("line");
const pendingXColumn = ref<string>(props.defaultXColumn ?? props.columns[0]?.name ?? "");
const pendingYColumns = ref<string[]>([]);

// ---------------------------------------------------------------------------
// Applied config + fetched data
// ---------------------------------------------------------------------------
const appliedConfig = ref<ChartConfig | null>(null);
const rows = ref<Record<string, unknown>[]>([]);
const fetching = ref(false);

// ---------------------------------------------------------------------------
// AG Charts options
// ---------------------------------------------------------------------------
const xAxisType = computed(() => {
  const dtype = props.columns.find(c => c.name === appliedConfig.value?.xColumn)?.dtype;
  if (dtype === "datetime" || dtype === "date") return "time";
  if (dtype === "integer" || dtype === "float" || dtype === "decimal") return "number";
  return "category";
});

// Convert date/datetime strings to JS Date objects for AG Charts time axis
const processedRows = computed(() => {
  if (!appliedConfig.value || rows.value.length === 0) return [];
  const xCol = appliedConfig.value.xColumn;
  const dtype = props.columns.find(c => c.name === xCol)?.dtype;
  if (dtype !== "datetime" && dtype !== "date") return rows.value;

  return rows.value.map(row => ({
    ...row,
    [xCol]: new Date((row[xCol] as string).replace(" ", "T")),
  }));
});

const chartOptions = computed((): AgChartOptions | undefined => {
  if (!appliedConfig.value) return undefined;
  const { chartType, xColumn, yColumns } = appliedConfig.value;

  // Build typed options — cast via unknown to satisfy AG Charts' discriminated union
  return {
    data: processedRows.value,
    series: yColumns.map(yCol => ({
      type: chartType,
      xKey: xColumn,
      yKey: yCol,
      title: yCol,
    })),
    axes: [
      { type: xAxisType.value, position: "bottom" },
      { type: "number", position: "left" },
    ],
  } as unknown as AgChartOptions;
});

// ---------------------------------------------------------------------------
// Apply / fetch
// ---------------------------------------------------------------------------
const canApply = computed(() =>
  pendingXColumn.value !== "" && pendingYColumns.value.length > 0
);

async function applyConfig() {
  if (!canApply.value) return;

  const config: ChartConfig = {
    chartType: pendingChartType.value,
    xColumn: pendingXColumn.value,
    yColumns: [...pendingYColumns.value],
  };

  fetching.value = true;
  try {
    const data = await invoke<Record<string, unknown>[]>("get_chart_data", {
      xCol: config.xColumn,
      yCols: config.yColumns,
      filters: props.activeFilters,
    });
    rows.value = data;
    appliedConfig.value = config;
  } catch (err) {
    alert(`Chart error: ${err}`);
  } finally {
    fetching.value = false;
  }
}

watch(
  () => props.activeFilters,
  () => { if (appliedConfig.value && !fetching.value) applyConfig(); },
  { deep: true }
);
</script>

<template>
  <div class="chart-view">
    <!-- Config bar -->
    <div class="chart-config-bar">
      <!-- Chart type toggle -->
      <div class="config-group">
        <span class="config-label">Type</span>
        <div class="type-toggle">
          <button
            v-for="t in chartTypes"
            :key="t.value"
            :class="{ active: pendingChartType === t.value }"
            @click="pendingChartType = t.value"
          >{{ t.label }}</button>
        </div>
      </div>

      <div class="config-sep" />

      <!-- X-axis column selector -->
      <div class="config-group">
        <span class="config-label">X-Axis</span>
        <select v-model="pendingXColumn" class="config-select">
          <option v-for="col in columns" :key="col.name" :value="col.name">{{ col.name }}</option>
        </select>
      </div>

      <div class="config-sep" />

      <!-- Y-axis multi-select (numeric columns only) -->
      <div class="config-group y-axis-group">
        <span class="config-label">Y-Axis</span>
        <div class="y-cols">
          <label
            v-for="col in numericColumns"
            :key="col.name"
            class="y-col-item"
          >
            <input type="checkbox" :value="col.name" v-model="pendingYColumns" />
            <span>{{ col.name }}</span>
          </label>
          <span v-if="numericColumns.length === 0" class="no-numeric">No numeric columns</span>
        </div>
      </div>

      <button
        class="btn-apply"
        :disabled="!canApply || fetching"
        @click="applyConfig"
      >Apply</button>
    </div>

    <!-- Chart area -->
    <div class="chart-area" :class="{ fetching }">
      <div v-if="!appliedConfig && !fetching" class="chart-empty">
        Select Y-axis columns and click Apply to generate a chart
      </div>
      <AgCharts
        v-else-if="appliedConfig"
        :options="chartOptions"
        class="chart-instance"
      />
    </div>
  </div>
</template>

<style scoped>
.chart-view {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

/* ---- Config bar ---- */
.chart-config-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 14px;
  background: var(--ag-header-background-color, #f8f8f8);
  border-bottom: 1px solid var(--ag-border-color, #babfc7);
  flex-shrink: 0;
  font-family: var(--ag-font-family, inherit);
  font-size: var(--ag-font-size, 14px);
}

.config-group {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.config-label {
  font-size: 0.75rem;
  color: var(--ag-disabled-foreground-color, #888);
  white-space: nowrap;
  font-weight: 500;
}

.config-sep {
  width: 1px;
  height: 18px;
  background: var(--ag-border-color, #babfc7);
  flex-shrink: 0;
}

/* Type toggle — mirrors App.vue .view-toggle */
.type-toggle {
  display: flex;
}

.type-toggle button {
  padding: 3px 10px;
  font-size: 0.8rem;
  font-family: var(--ag-font-family, inherit);
  background: var(--ag-background-color, #fff);
  color: var(--ag-foreground-color, #444);
  border: 1px solid var(--ag-border-color, #babfc7);
  border-radius: 0;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.type-toggle button:first-child {
  border-radius: var(--ag-border-radius, 4px) 0 0 var(--ag-border-radius, 4px);
}

.type-toggle button:last-child {
  border-radius: 0 var(--ag-border-radius, 4px) var(--ag-border-radius, 4px) 0;
  border-left: none;
}

.type-toggle button:hover:not(.active) {
  background: var(--ag-row-hover-color, rgba(33, 150, 243, 0.12));
}

.type-toggle button.active {
  background: var(--ag-active-color, #2196f3);
  color: #fff;
  border-color: var(--ag-active-color, #2196f3);
}

/* X-axis dropdown */
.config-select {
  padding: 3px 6px;
  font-size: 0.8rem;
  font-family: var(--ag-font-family, inherit);
  border: 1px solid var(--ag-input-border-color, #babfc7);
  border-radius: var(--ag-border-radius, 4px);
  background: var(--ag-background-color, #fff);
  color: var(--ag-foreground-color, #181d1f);
  cursor: pointer;
  max-width: 200px;
}

.config-select:focus {
  outline: none;
  border-color: var(--ag-input-focus-border-color, #2196f3);
  box-shadow: var(--ag-input-focus-box-shadow);
}

/* Y-axis column checkboxes */
.y-axis-group {
  flex: 1;
  overflow: hidden;
}

.y-cols {
  display: flex;
  align-items: center;
  gap: 10px;
  overflow-x: auto;
  padding: 2px 0;
  scrollbar-width: thin;
}

.y-col-item {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 0.8rem;
  white-space: nowrap;
  cursor: pointer;
  user-select: none;
}

.y-col-item input[type="checkbox"] {
  accent-color: var(--ag-active-color, #2196f3);
  cursor: pointer;
}

.no-numeric {
  font-size: 0.8rem;
  color: var(--ag-disabled-foreground-color, #888);
  font-style: italic;
}

/* Apply button — .btn-apply base styles come from shared.css (globally imported) */
.btn-apply {
  flex-shrink: 0;
  white-space: nowrap;
}

/* ---- Chart area ---- */
.chart-area {
  flex: 1;
  position: relative;
  overflow: hidden;
  min-height: 0;
}

.chart-instance {
  width: 100%;
  height: 100%;
}

.chart-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--ag-disabled-foreground-color, #888);
  font-size: 0.9rem;
  font-family: var(--ag-font-family, inherit);
}
</style>
