<script setup lang="ts">
import type { OverviewNode } from "../overview";
import TreeNode from "./TreeNode.vue";

defineProps<{ root: OverviewNode }>();
const emit = defineEmits<{ close: [] }>();
</script>

<template>
  <aside class="overview-panel">
    <header class="header">
      <span class="title">Structure</span>
      <button class="close" type="button" title="Close overview" @click="emit('close')">✕</button>
    </header>
    <div class="tree">
      <TreeNode :node="root" />
    </div>
  </aside>
</template>

<style scoped>
.overview-panel {
  flex: none;
  width: 320px;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--vw-surface);
  border-left: 1px solid var(--vw-border);
}

.header {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--vw-space-2) var(--vw-space-3);
  border-bottom: 1px solid var(--vw-border);
}

.title {
  color: var(--vw-fg);
  font-size: var(--vw-fs-sm);
  font-weight: 600;
}

.close {
  border: none;
  background: transparent;
  color: var(--vw-fg-muted);
  cursor: pointer;
  font-size: var(--vw-fs-sm);
  line-height: 1;
  padding: 2px 4px;
  border-radius: var(--vw-radius-sm);
}
.close:hover {
  background: var(--vw-surface-2);
  color: var(--vw-fg);
}

.tree {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--vw-space-2) 0;
}
</style>
