import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

/**
 * Build a Vite config tailored for a Tauri v2 + Vue 3 viewer app.
 *
 * A factory, not a static config: each app passes its own vendor `manualChunks`. Common
 * Tauri plumbing (fixed dev port, HMR over the dev host, ignoring `src-tauri`, not clearing
 * the screen so Rust errors survive) is applied here.
 *
 * Authored as plain JS (types live in `vite.d.ts`): this module is imported by each app's
 * Vite config, which Vite externalizes to Node at runtime. Shipping raw `.ts` here would
 * only load on Node versions that strip types, breaking `vite build` on Node 20.
 *
 * @param {import("./vite.js").ViewerConfigOptions} [opts]
 */
export function viewerConfig(opts = {}) {
  // @tauri-apps/cli sets this when running `tauri dev` on a mobile/remote host.
  const host = process.env.TAURI_DEV_HOST;

  return defineConfig(async () => {
    /** @type {import("vite").UserConfig} */
    const config = {
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
