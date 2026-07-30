import { defineConfig, type UserConfig } from "vite";
import vue from "@vitejs/plugin-vue";

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
 *
 * A factory, not a static config: each app passes its own vendor `manualChunks`. Common
 * Tauri plumbing (fixed dev port, HMR over the dev host, ignoring `src-tauri`, not clearing
 * the screen so Rust errors survive) is applied here.
 */
export function viewerConfig(opts: ViewerConfigOptions = {}) {
  // @tauri-apps/cli sets this when running `tauri dev` on a mobile/remote host.
  const host = process.env.TAURI_DEV_HOST;

  return defineConfig(async () => {
    const config: UserConfig = {
      plugins: [vue(), ...(opts.plugins ?? [])],

      // Prevent Vite from obscuring Rust errors.
      clearScreen: false,

      build: opts.manualChunks
        ? { rollupOptions: { output: { manualChunks: opts.manualChunks } } }
        : {},

      // Tauri expects a fixed port and fails if it is not available.
      server: {
        port: 1420,
        strictPort: true,
        host: host || false,
        hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
        watch: {
          // Don't watch the Rust side.
          ignored: ["**/src-tauri/**"],
        },
      },

      // Tauri uses Chromium on Windows and WebKit on macOS/Linux.
      envPrefix: ["VITE_", "TAURI_ENV_"],

      ...opts.overrides,
    };
    return config;
  });
}
