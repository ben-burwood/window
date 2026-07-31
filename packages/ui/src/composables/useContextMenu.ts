import { ref } from "vue";

// Menu open state and cursor position for a right-click context menu. Wire
// openMenu to `@contextmenu.prevent` and pass open/x/y to <ContextMenu>.
export function useContextMenu() {
  const open = ref(false);
  const x = ref(0);
  const y = ref(0);

  function openMenu(e: MouseEvent) {
    e.preventDefault();
    x.value = e.clientX;
    y.value = e.clientY;
    open.value = true;
  }

  function close() {
    open.value = false;
  }

  return { open, x, y, openMenu, close };
}
