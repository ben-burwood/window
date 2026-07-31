import { computed, ref } from "vue";
import type maplibregl from "maplibre-gl";

export const BASEMAPS = {
  bright: { label: "Bright", url: "https://tiles.openfreemap.org/styles/bright" },
  positron: { label: "Positron (light)", url: "https://tiles.openfreemap.org/styles/positron" },
} as const;

export type Basemap = keyof typeof BASEMAPS;

// Structurally assignable to @window/ui's ContextMenuItem — kept local so this
// map package doesn't have to depend on the UI package.
export interface BasemapMenuItem {
  id: Basemap;
  label: string;
  checked: boolean;
}

export interface UseBasemapOptions {
  /** Returns the live MapLibre instance (held outside Vue reactivity by callers). */
  getMap: () => maplibregl.Map | null;
  /** Source ids to carry across a basemap switch. Resolved lazily at switch time. */
  overlaySourceIds: () => Iterable<string>;
  /** Layer ids to carry across a basemap switch. Resolved lazily (may be dynamic). */
  overlayLayerIds: () => Iterable<string>;
  /** Basemap shown first. Defaults to "bright". */
  initial?: Basemap;
}

// Owns the current basemap and switches the base style without disturbing the
// caller's data overlay. Menu open/position state is left to the host (see
// @window/ui's useContextMenu) so the basemap items can be combined with other
// menu entries such as the theme toggle.
//
// These are full style documents, so we switch with setStyle rather than
// toggling layer visibility. transformStyle re-merges the caller's overlay
// sources/layers (by id) from the outgoing style into the incoming one, with
// layers appended last so they stay on top of the new base map.
export function useBasemap(opts: UseBasemapOptions) {
  const current = ref<Basemap>(opts.initial ?? "bright");

  const styleUrl = computed(() => BASEMAPS[current.value].url);
  const items = computed<BasemapMenuItem[]>(() =>
    (Object.keys(BASEMAPS) as Basemap[]).map((id) => ({
      id,
      label: BASEMAPS[id].label,
      checked: current.value === id,
    })),
  );

  function setBasemap(name: Basemap) {
    const map = opts.getMap();
    if (!map || name === current.value) return;
    current.value = name;
    const sourceIds = new Set(opts.overlaySourceIds());
    const layerIds = new Set(opts.overlayLayerIds());
    map.setStyle(BASEMAPS[name].url, {
      transformStyle: (prev, next) => {
        if (!prev) return next;
        const sources = { ...next.sources };
        for (const id of sourceIds) if (prev.sources[id]) sources[id] = prev.sources[id];
        const layers = [...next.layers, ...prev.layers.filter((l) => layerIds.has(l.id))];
        return { ...next, sources, layers };
      },
    });
  }

  // Applies the selection if `id` names a basemap; returns whether it did, so a
  // host can chain this with other menu-item handlers.
  function select(id: string): boolean {
    if (!(id in BASEMAPS)) return false;
    setBasemap(id as Basemap);
    return true;
  }

  return { current, styleUrl, items, select, setBasemap };
}
