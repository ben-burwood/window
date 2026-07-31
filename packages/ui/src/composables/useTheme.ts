import { computed, ref } from "vue";
import type { ContextMenuItem } from "../components/ContextMenu.vue";

export type Theme = "light" | "dark";

const STORAGE_KEY = "vw-theme";
// Stable id so a host can route the theme item's selection through handleSelect.
const THEME_ITEM_ID = "vw:theme-toggle";

function readStored(): Theme | null {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return v === "light" || v === "dark" ? v : null;
  } catch {
    return null;
  }
}

function systemTheme(): Theme {
  return typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

// Reflect the choice onto <html data-theme> so theme.css can override the
// prefers-color-scheme default. Applied to the root so it also covers the
// context menu, which teleports to <body>.
function apply(t: Theme) {
  document.documentElement.setAttribute("data-theme", t);
}

// Module-level singleton: every useTheme() call across an app shares one source
// of truth, so a toggle in one place updates every menu at once.
const theme = ref<Theme>(readStored() ?? systemTheme());
let applied = false;

// Light/dark theme with a persisted manual override. The initial value follows
// the OS unless the user has toggled before (stored in localStorage).
export function useTheme() {
  if (!applied) {
    apply(theme.value);
    applied = true;
  }

  const isDark = computed(() => theme.value === "dark");

  function setTheme(t: Theme) {
    theme.value = t;
    apply(t);
    try {
      localStorage.setItem(STORAGE_KEY, t);
    } catch {
      // Storage may be unavailable (private mode etc.) — the in-memory value still applies.
    }
  }

  function toggle() {
    setTheme(isDark.value ? "light" : "dark");
  }

  // Ready-made context-menu item so every app shows an identical toggle.
  const menuItem = computed<ContextMenuItem>(() => ({
    id: THEME_ITEM_ID,
    label: "Dark mode",
    checked: isDark.value,
  }));

  // Returns true if the id was the theme toggle (and handled it), so hosts can
  // chain this ahead of their own menu-item handling.
  function handleSelect(id: string): boolean {
    if (id !== THEME_ITEM_ID) return false;
    toggle();
    return true;
  }

  return { theme, isDark, setTheme, toggle, menuItem, handleSelect };
}
