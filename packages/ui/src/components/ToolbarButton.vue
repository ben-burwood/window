<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: "primary" | "default";
    active?: boolean;
    disabled?: boolean;
    title?: string;
  }>(),
  { variant: "default", active: false, disabled: false },
);

const emit = defineEmits<{ (e: "click"): void }>();
</script>

<template>
  <button
    class="vw-btn"
    :class="{ 'vw-btn--primary': variant === 'primary', 'is-active': active }"
    :disabled="disabled"
    :title="title"
    type="button"
    @click="emit('click')"
  >
    <slot />
  </button>
</template>

<style scoped>
.is-active {
  background: var(--vw-accent-weak);
  border-color: var(--vw-accent);
  color: var(--vw-accent);
}
</style>
