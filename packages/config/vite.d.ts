import type { UserConfig, UserConfigExport } from "vite";

/**
 * Options each app passes to {@link viewerConfig}. Everything is optional; the factory
 * supplies the Tauri-tailored defaults.
 */
export interface ViewerConfigOptions {
  /**
   * Rollup `manualChunks` for this app's heavy vendor libs, e.g.
   * `{ "vendor-maplibre": ["maplibre-gl"] }`. Each app owns its own lazy-chunk split.
   */
  manualChunks?: Record<string, string[]>;
  /** Extra Vite plugins to append after `@vitejs/plugin-vue`. */
  plugins?: UserConfig["plugins"];
  /** Escape hatch: shallow-merged over the computed config (wins on top-level keys). */
  overrides?: UserConfig;
}

/**
 * Build a Vite config tailored for a Tauri v2 + Vue 3 viewer app.
 */
export declare function viewerConfig(opts?: ViewerConfigOptions): UserConfigExport;
