<script setup lang="ts">
// Format-agnostic launch / empty screen. Knows nothing about file formats: the title, hint,
// error text and drag state are all passed in; it only emits `open` when the button is
// clicked. The app owns the actual OS drag-drop wiring (via @window/bridge `onFileDrop`) and
// passes `dragging` / `error` back down.
withDefaults(
  defineProps<{
    title: string;
    hint?: string;
    error?: string | null;
    dragging?: boolean;
    actionLabel?: string;
  }>(),
  { actionLabel: "Open File" },
);

const emit = defineEmits<{ (e: "open"): void }>();
</script>

<template>
  <div class="vw-empty" :class="{ 'vw-empty--dragging': dragging }">
    <div class="vw-empty__card">
      <slot name="icon">
        <svg
          class="vw-empty__icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path
            d="M14.25 2.25H6.75A1.5 1.5 0 0 0 5.25 3.75v16.5a1.5 1.5 0 0 0 1.5 1.5h10.5a1.5 1.5 0 0 0 1.5-1.5V6.75Z"
          />
          <path d="M14.25 2.25v4.5h4.5" />
        </svg>
      </slot>
      <h1 class="vw-empty__title">{{ title }}</h1>
      <p v-if="error" class="vw-empty__hint vw-empty__hint--error">{{ error }}</p>
      <p v-else-if="hint" class="vw-empty__hint">{{ hint }}</p>
      <button class="vw-btn vw-btn--primary vw-empty__action" @click="emit('open')">
        {{ actionLabel }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.vw-empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--vw-space-5);
  background: var(--vw-bg);
}

.vw-empty__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: var(--vw-space-3);
  max-width: 360px;
  padding: var(--vw-space-6);
  border: 1.5px dashed var(--vw-border-strong);
  border-radius: var(--vw-radius-lg);
  transition:
    border-color 0.15s ease,
    background 0.15s ease;
}

.vw-empty--dragging .vw-empty__card {
  border-color: var(--vw-accent);
  background: var(--vw-accent-weak);
}

.vw-empty__icon {
  width: 44px;
  height: 44px;
  color: var(--vw-fg-subtle);
}

.vw-empty__title {
  margin: 0;
  font-size: var(--vw-fs-xl);
  font-weight: 600;
  color: var(--vw-fg);
}

.vw-empty__hint {
  margin: 0;
  font-size: var(--vw-fs-md);
  color: var(--vw-fg-muted);
}

.vw-empty__hint--error {
  color: var(--vw-danger);
}

.vw-empty__action {
  margin-top: var(--vw-space-2);
}
</style>
