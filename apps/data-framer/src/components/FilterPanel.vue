<script setup lang="ts">
import { ref } from "vue";
import type { ColumnInfo, Dtype, FilterSpec } from "../types";

type FilterOp =
  | "eq" | "neq"
  | "gt" | "gte" | "lt" | "lte" | "between"
  | "contains" | "not_contains" | "starts_with" | "ends_with"
  | "is_true" | "is_false"
  | "is_null" | "is_not_null";

interface OpDef {
  label: string;
  op: FilterOp;
  hasValue: boolean;
  hasTwoValues: boolean;
}

const props = defineProps<{ columns: ColumnInfo[] }>();
const emit = defineEmits<{
  apply: [filters: FilterSpec[]];
  clear: [];
}>();

// ---------------------------------------------------------------------------
// Op definitions
// ---------------------------------------------------------------------------
const NULL_OPS: OpDef[] = [
  { label: "is null",     op: "is_null",     hasValue: false, hasTwoValues: false },
  { label: "is not null", op: "is_not_null", hasValue: false, hasTwoValues: false },
];

const NUMERIC_OPS: OpDef[] = [
  { label: "equals",                op: "eq",      hasValue: true,  hasTwoValues: false },
  { label: "not equals",            op: "neq",     hasValue: true,  hasTwoValues: false },
  { label: "greater than",          op: "gt",      hasValue: true,  hasTwoValues: false },
  { label: "greater than or equal", op: "gte",     hasValue: true,  hasTwoValues: false },
  { label: "less than",             op: "lt",      hasValue: true,  hasTwoValues: false },
  { label: "less than or equal",    op: "lte",     hasValue: true,  hasTwoValues: false },
  { label: "between",               op: "between", hasValue: true,  hasTwoValues: true  },
  ...NULL_OPS,
];

const DATE_OPS: OpDef[] = [
  { label: "equals",       op: "eq",      hasValue: true,  hasTwoValues: false },
  { label: "not equals",   op: "neq",     hasValue: true,  hasTwoValues: false },
  { label: "after",        op: "gt",      hasValue: true,  hasTwoValues: false },
  { label: "after or on",  op: "gte",     hasValue: true,  hasTwoValues: false },
  { label: "before",       op: "lt",      hasValue: true,  hasTwoValues: false },
  { label: "before or on", op: "lte",     hasValue: true,  hasTwoValues: false },
  { label: "between",      op: "between", hasValue: true,  hasTwoValues: true  },
  ...NULL_OPS,
];

const OPS_BY_DTYPE: Record<Dtype, OpDef[]> = {
  string: [
    { label: "equals",       op: "eq",          hasValue: true,  hasTwoValues: false },
    { label: "not equals",   op: "neq",         hasValue: true,  hasTwoValues: false },
    { label: "contains",     op: "contains",    hasValue: true,  hasTwoValues: false },
    { label: "not contains", op: "not_contains",hasValue: true,  hasTwoValues: false },
    { label: "starts with",  op: "starts_with", hasValue: true,  hasTwoValues: false },
    { label: "ends with",    op: "ends_with",   hasValue: true,  hasTwoValues: false },
    ...NULL_OPS,
  ],
  integer:  NUMERIC_OPS,
  float:    NUMERIC_OPS,
  boolean: [
    { label: "is true",  op: "is_true",  hasValue: false, hasTwoValues: false },
    { label: "is false", op: "is_false", hasValue: false, hasTwoValues: false },
    ...NULL_OPS,
  ],
  date:     DATE_OPS,
  datetime: DATE_OPS,
  decimal:  NUMERIC_OPS,
  categorical: NULL_OPS,
  time:        NULL_OPS,
  duration:    NULL_OPS,
  binary:      NULL_OPS,
  list:        NULL_OPS,
  struct:      NULL_OPS,
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
const pendingFilters = ref<FilterSpec[]>([]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function opDefsForFilter(f: FilterSpec): OpDef[] {
  const col = props.columns.find((c) => c.name === f.column);
  return col ? OPS_BY_DTYPE[col.dtype] ?? [] : [];
}

function currentOpDef(f: FilterSpec): OpDef | undefined {
  return opDefsForFilter(f).find((d) => d.op === f.op);
}

function onColumnChange(f: FilterSpec) {
  const defs = opDefsForFilter(f);
  if (defs.length > 0) {
    f.op = defs[0].op;
    f.value = "";
    f.value2 = "";
  }
}

function inputTypeForFilter(f: FilterSpec): string {
  const col = props.columns.find((c) => c.name === f.column);
  switch (col?.dtype) {
    case "integer":
    case "float":
    case "decimal":  return "number";
    case "date":     return "date";
    case "datetime": return "datetime-local";
    default:         return "text";
  }
}

function addFilter() {
  const firstCol = props.columns[0];
  if (!firstCol) return;
  pendingFilters.value.push({
    column: firstCol.name,
    op: OPS_BY_DTYPE[firstCol.dtype][0].op,
    value: "",
    value2: "",
  });
}

function removeFilter(idx: number) {
  pendingFilters.value.splice(idx, 1);
}

function normalizeDateTime(value: string): string {
  return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value) ? value + ":00" : value;
}

// ---------------------------------------------------------------------------
// Apply / clear
// ---------------------------------------------------------------------------
function applyFilters() {
  for (const f of pendingFilters.value) {
    const def = currentOpDef(f);
    if (!def) continue;
    if ((def.hasValue && !String(f.value ?? "").trim()) || (def.hasTwoValues && !String(f.value2 ?? "").trim())) {
      alert("Please fill in all filter values before applying.");
      return;
    }
  }

  const dtype = (col: string) => props.columns.find((c) => c.name === col)?.dtype ?? "string";
  const filters: FilterSpec[] = pendingFilters.value.map((f) => {
    const val  = String(f.value  ?? "");
    const val2 = String(f.value2 ?? "");
    const isDatetime = dtype(f.column) === "datetime";
    return {
      ...f,
      value:  isDatetime ? normalizeDateTime(val)  : val,
      value2: isDatetime ? normalizeDateTime(val2) : val2,
    };
  });

  emit("apply", filters);
}

function clearFilters() {
  pendingFilters.value = [];
  emit("clear");
}
</script>

<template>
  <div class="panel">
    <div v-for="(f, i) in pendingFilters" :key="`${i}_${f.column}`" class="filter-row">
      <select v-model="f.column" @change="onColumnChange(f)" class="filter-select">
        <option v-for="col in columns" :key="col.name" :value="col.name">
          {{ col.name }}
        </option>
      </select>
      <select v-model="f.op" class="filter-select op-select">
        <option v-for="def in opDefsForFilter(f)" :key="def.op" :value="def.op">
          {{ def.label }}
        </option>
      </select>
      <input
        v-if="currentOpDef(f)?.hasValue"
        v-model="f.value"
        :type="inputTypeForFilter(f)"
        class="filter-value"
        placeholder="value"
      />
      <input
        v-if="currentOpDef(f)?.hasTwoValues"
        v-model="f.value2"
        :type="inputTypeForFilter(f)"
        class="filter-value"
        placeholder="to"
      />
      <button class="remove-btn" @click="removeFilter(i)" title="Remove filter">×</button>
    </div>
    <div class="panel-actions">
      <button class="add-filter-btn" @click="addFilter">+ Add Filter</button>
      <div class="panel-action-btns">
        <button class="btn-secondary" @click="clearFilters">Clear All</button>
        <button class="btn-apply" @click="applyFilters">Apply</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.filter-row {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.filter-select {
  padding: 4px 6px;
  font-size: 0.8rem;
  font-family: var(--ag-font-family, inherit);
  border: 1px solid var(--ag-input-border-color, var(--ag-border-color, #babfc7));
  border-radius: var(--ag-border-radius, 4px);
  background: var(--ag-background-color, #fff);
  color: var(--ag-foreground-color, #181d1f);
  cursor: pointer;
}

.filter-select:focus,
.filter-value:focus {
  outline: none;
  border-color: var(--ag-input-focus-border-color, var(--ag-active-color, #2196f3));
  box-shadow: var(--ag-input-focus-box-shadow, 0 0 0 3px color-mix(in srgb, transparent, var(--ag-active-color, #2196f3) 47%));
}

.filter-select.op-select {
  min-width: 120px;
}

.filter-value {
  padding: 4px 8px;
  font-size: 0.8rem;
  font-family: var(--ag-font-family, inherit);
  border: 1px solid var(--ag-input-border-color, var(--ag-border-color, #babfc7));
  border-radius: var(--ag-border-radius, 4px);
  color: var(--ag-foreground-color, #181d1f);
  background: var(--ag-background-color, #fff);
  width: 160px;
}

.remove-btn {
  padding: 2px 8px;
  font-size: 1rem;
  line-height: 1;
  background: transparent;
  color: var(--ag-disabled-foreground-color, #999);
  border: 1px solid var(--ag-border-color, #babfc7);
  border-radius: var(--ag-border-radius, 4px);
  cursor: pointer;
}

.remove-btn:hover:not(:disabled) {
  background: #fee2e2;
  color: #b91c1c;
  border-color: #fca5a5;
}

.add-filter-btn {
  padding: 4px 12px;
  font-size: 0.8rem;
  font-family: var(--ag-font-family, inherit);
  background: transparent;
  color: var(--ag-active-color, #2196f3);
  border: 1px dashed var(--ag-active-color, #2196f3);
  border-radius: var(--ag-border-radius, 4px);
  cursor: pointer;
}

.add-filter-btn:hover:not(:disabled) {
  background: color-mix(in srgb, transparent, var(--ag-active-color, #2196f3) 10%);
}
</style>
