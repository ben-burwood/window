import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

/**
 * Turn `{ chunkName: [packages] }` into a `manualChunks` function. Vite 8 (Rolldown) only
 * accepts the function form; the object form fails with "manualChunks is not a function".
 *
 * @param {Record<string, string[]>} chunks
 * @returns {(id: string) => string | undefined}
 */
function chunkByPackage(chunks) {
  const entries = Object.entries(chunks).flatMap(([name, pkgs]) =>
    pkgs.map((pkg) => /** @type {const} */ ([`/node_modules/${pkg}/`, name])),
  );
  return (id) => {
    const path = id.replace(/\\/g, "/");
    return entries.find(([needle]) => path.includes(needle))?.[1];
  };
}

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
        ? { rollupOptions: { output: { manualChunks: chunkByPackage(opts.manualChunks) } } }
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
