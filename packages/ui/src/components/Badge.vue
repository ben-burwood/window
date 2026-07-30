<script setup lang="ts">
// Tokenized status pill; label comes from the slot. Set `interactive` to make it a clickable,
// keyboard-focusable button that emits `click`.
withDefaults(
  defineProps<{
    variant?: "default" | "warning" | "danger";
    interactive?: boolean;
  }>(),
  { variant: "default", interactive: false },
);

const emit = defineEmits<{ (e: "click"): void }>();
</script>

<template>
  <span
    class="vw-badge"
    :class="[`vw-badge--${variant}`, { 'vw-badge--interactive': interactive }]"
    :role="interactive ? 'button' : undefined"
    :tabindex="interactive ? 0 : undefined"
    @click="interactive && emit('click')"
    @keyup.enter="interactive && emit('click')"
  >
    <slot />
  </span>
</template>

<style scoped>
.vw-badge {
  display: inline-flex;
  align-items: center;
  padding: 0 var(--vw-space-2);
  height: 18px;
  border-radius: 999px;
  border: 1px solid transparent;
  font-size: var(--vw-fs-xs);
  font-weight: 600;
  line-height: 1;
  text-transform: uppercase;
  letter-spacing: 0.02em;
  white-space: nowrap;
}

.vw-badge--interactive {
  cursor: pointer;
}
.vw-badge--interactive:hover {
  border-color: currentColor;
}

.vw-badge--default {
  background: var(--vw-surface-2);
  color: var(--vw-fg-muted);
  border-color: var(--vw-border);
}

.vw-badge--warning {
  background: color-mix(in srgb, var(--vw-danger) 12%, transparent);
  color: var(--vw-danger);
  border-color: color-mix(in srgb, var(--vw-danger) 35%, transparent);
}

.vw-badge--danger {
  background: var(--vw-danger);
  color: #fff;
  border-color: var(--vw-danger);
}
</style>
