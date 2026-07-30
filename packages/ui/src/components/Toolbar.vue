<template>
  <header class="vw-toolbar">
    <div class="vw-toolbar__start">
      <slot name="start"><slot /></slot>
    </div>
    <div class="vw-toolbar__center">
      <slot name="center" />
    </div>
    <div class="vw-toolbar__end">
      <slot name="end" />
    </div>
  </header>
</template>

<style scoped>
.vw-toolbar {
  display: flex;
  align-items: center;
  gap: var(--vw-space-3);
  height: 48px;
  flex: 0 0 auto;
  padding: 0 var(--vw-space-3);
  background: var(--vw-surface);
  border-bottom: 1px solid var(--vw-border);
}

.vw-toolbar__start {
  display: flex;
  align-items: center;
  gap: var(--vw-space-2);
  min-width: 0;
}

.vw-toolbar__center {
  display: flex;
  align-items: center;
  gap: var(--vw-space-2);
  flex: 1 1 auto;
  justify-content: center;
  min-width: 0;
}

.vw-toolbar__end {
  display: flex;
  align-items: center;
  gap: var(--vw-space-2);
  margin-left: auto;
}
</style>
