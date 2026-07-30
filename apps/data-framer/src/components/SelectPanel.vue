<script setup lang="ts">
import { ref, onMounted } from "vue";
import type { ColumnInfo } from "../types";

const props = defineProps<{
  columns: ColumnInfo[];
  activeColumnVisibility: Record<string, boolean>;
}>();

const emit = defineEmits<{
  apply: [visibility: Record<string, boolean>];
  reset: [];
}>();

const pendingColumnVisibility = ref<Record<string, boolean>>({});

onMounted(() => {
  pendingColumnVisibility.value = { ...props.activeColumnVisibility };
});

function togglePendingColumn(name: string) {
  pendingColumnVisibility.value[name] = !pendingColumnVisibility.value[name];
}

function selectAllColumns() {
  pendingColumnVisibility.value = Object.fromEntries(props.columns.map(c => [c.name, true]));
}

function deselectAllColumns() {
  pendingColumnVisibility.value = Object.fromEntries(props.columns.map(c => [c.name, false]));
}

function applyColumns() {
  const anyVisible = props.columns.some(c => pendingColumnVisibility.value[c.name] !== false);
  if (!anyVisible) { alert("At least one column must be visible."); return; }
  emit("apply", { ...pendingColumnVisibility.value });
}

function resetColumns() {
  const vis = Object.fromEntries(props.columns.map(c => [c.name, true]));
  pendingColumnVisibility.value = vis;
  emit("reset");
}
</script>

<template>
  <div class="panel">
    <div class="column-list">
      <label v-for="col in columns" :key="col.name" class="column-item">
        <input
          type="checkbox"
          :checked="pendingColumnVisibility[col.name] !== false"
          @change="togglePendingColumn(col.name)"
        />
        <span class="col-item-name">{{ col.name }}</span>
        <span class="col-dtype-badge">{{ col.dtype }}</span>
      </label>
    </div>
    <div class="panel-actions">
      <div class="panel-action-btns">
        <button class="btn-secondary" @click="selectAllColumns">Select All</button>
        <button class="btn-secondary" @click="deselectAllColumns">Deselect All</button>
      </div>
      <div class="panel-action-btns">
        <button class="btn-secondary" @click="resetColumns">Reset</button>
        <button class="btn-apply" @click="applyColumns">Apply</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.column-list {
  max-height: 240px;
  overflow-y: auto;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 24px;
}

.column-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.82rem;
  cursor: pointer;
  min-width: 160px;
}

.col-item-name {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.col-dtype-badge {
  font-size: 0.7rem;
  color: var(--ag-disabled-foreground-color, #888);
  background: var(--ag-chip-background-color, color-mix(in srgb, transparent, var(--ag-foreground-color, #181d1f) 7%));
  border-radius: var(--ag-border-radius, 4px);
  padding: 1px 5px;
  white-space: nowrap;
}
</style>
