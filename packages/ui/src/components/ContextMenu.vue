<script lang="ts">
export interface ContextMenuItem {
  id: string;
  label?: string;
  /** Show a check mark in the left gutter (for toggle/radio-style items). */
  checked?: boolean;
  disabled?: boolean;
  /** Render a non-interactive divider instead of a menu item. */
  separator?: boolean;
}
</script>

<script setup lang="ts">
// A controlled right-click menu. The parent owns open state and position:
// open it from a `@contextmenu.prevent` handler by setting `x`/`y` to the
// event's clientX/clientY and `open` to true. Emits `select` with the item id
// and `close` on Escape, outside-click, scroll, resize, or window blur.
import { nextTick, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  x: number;
  y: number;
  items: ContextMenuItem[];
}>();

const emit = defineEmits<{ select: [id: string]; close: [] }>();

const menu = ref<HTMLElement | null>(null);
// Clamped position — kept off the reactive props so we can nudge it inside the
// viewport after the menu has real dimensions.
const left = ref(0);
const top = ref(0);

function clampToViewport() {
  const el = menu.value;
  if (!el) return;
  const { offsetWidth: w, offsetHeight: h } = el;
  const margin = 4;
  left.value = Math.min(props.x, window.innerWidth - w - margin);
  top.value = Math.min(props.y, window.innerHeight - h - margin);
}

function onPointerDown(e: PointerEvent) {
  if (menu.value && !menu.value.contains(e.target as Node)) emit("close");
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

function closeOnViewportChange() {
  emit("close");
}

function addListeners() {
  // Capture phase so an outside pointerdown closes us before it does anything else.
  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", closeOnViewportChange);
  window.addEventListener("blur", closeOnViewportChange);
  // A scroll anywhere would leave the menu detached from its anchor point.
  window.addEventListener("scroll", closeOnViewportChange, true);
}

function removeListeners() {
  document.removeEventListener("pointerdown", onPointerDown, true);
  document.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", closeOnViewportChange);
  window.removeEventListener("blur", closeOnViewportChange);
  window.removeEventListener("scroll", closeOnViewportChange, true);
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      left.value = props.x;
      top.value = props.y;
      addListeners();
      await nextTick();
      clampToViewport();
    } else {
      removeListeners();
    }
  },
);

function choose(item: ContextMenuItem) {
  if (item.disabled) return;
  emit("select", item.id);
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="menu"
      class="vw-context-menu"
      role="menu"
      :style="{ left: `${left}px`, top: `${top}px` }"
    >
      <template v-for="item in items" :key="item.id">
        <div v-if="item.separator" class="vw-context-menu__sep" role="separator"></div>
        <button
          v-else
          class="vw-context-menu__item"
          :class="{ 'is-checked': item.checked }"
          type="button"
          role="menuitemradio"
          :aria-checked="item.checked ? 'true' : 'false'"
          :disabled="item.disabled"
          @click="choose(item)"
        >
          <span class="vw-context-menu__check" aria-hidden="true">{{ item.checked ? "✓" : "" }}</span>
          <span class="vw-context-menu__label">{{ item.label }}</span>
        </button>
      </template>
    </div>
  </Teleport>
</template>

<style scoped>
.vw-context-menu {
  position: fixed;
  z-index: 1000;
  min-width: 180px;
  padding: var(--vw-space-1);
  background: var(--vw-surface);
  border: 1px solid var(--vw-border);
  border-radius: var(--vw-radius-sm);
  box-shadow: var(--vw-shadow);
  font-family: var(--vw-font);
}

.vw-context-menu__item {
  display: flex;
  align-items: center;
  gap: var(--vw-space-2);
  width: 100%;
  padding: var(--vw-space-1) var(--vw-space-2);
  border: none;
  border-radius: var(--vw-radius-sm);
  background: transparent;
  color: var(--vw-fg);
  font: inherit;
  font-size: var(--vw-fs-sm);
  text-align: left;
  cursor: pointer;
}

.vw-context-menu__item:hover:not(:disabled),
.vw-context-menu__item:focus-visible {
  background: var(--vw-surface-2);
  outline: none;
}

.vw-context-menu__item:disabled {
  opacity: 0.5;
  cursor: default;
}

.vw-context-menu__item.is-checked {
  color: var(--vw-accent);
}

.vw-context-menu__check {
  flex: 0 0 auto;
  width: 12px;
  text-align: center;
  color: var(--vw-accent);
}

.vw-context-menu__label {
  flex: 1 1 auto;
}

.vw-context-menu__sep {
  height: 1px;
  margin: var(--vw-space-1) 0;
  background: var(--vw-border);
}
</style>
