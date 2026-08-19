<script setup lang="ts">
import { ref } from "vue";
import type { OverviewNode } from "../overview";

// Recursive: renders itself for each child. `name` lets the template reference
// the component by <TreeNode>.
defineOptions({ name: "TreeNode" });

const props = withDefaults(defineProps<{ node: OverviewNode; depth?: number }>(), { depth: 0 });

const expandable = Array.isArray(props.node.children);
const expanded = ref(props.node.defaultOpen ?? false);

function toggle() {
  if (expandable) expanded.value = !expanded.value;
}
</script>

<template>
  <div
    class="row"
    :class="{ expandable }"
    :style="{ paddingLeft: `${depth * 14 + 6}px` }"
    @click="toggle"
  >
    <span class="caret">{{ expandable ? (expanded ? "▾" : "▸") : "" }}</span>
    <span class="label">{{ node.label }}</span>
    <span v-if="node.detail" class="detail">{{ node.detail }}</span>
  </div>

  <template v-if="expandable && expanded">
    <TreeNode v-for="(child, i) in node.children" :key="i" :node="child" :depth="depth + 1" />
  </template>
</template>

<style scoped>
.row {
  display: flex;
  align-items: baseline;
  gap: var(--vw-space-2);
  padding-top: 2px;
  padding-bottom: 2px;
  padding-right: var(--vw-space-2);
  cursor: default;
  white-space: nowrap;
}
.row.expandable {
  cursor: pointer;
}
.row.expandable:hover {
  background: var(--vw-surface-2);
}

.caret {
  flex: none;
  width: 12px;
  color: var(--vw-fg-subtle);
  font-size: 10px;
}

.label {
  color: var(--vw-fg);
  font-size: var(--vw-fs-sm);
}

.detail {
  color: var(--vw-fg-muted);
  font-family: var(--vw-font-mono);
  font-size: var(--vw-fs-xs);
}
</style>
