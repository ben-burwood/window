import { viewerConfig } from "@viewers/config/vite";

// https://vitejs.dev/config/
export default viewerConfig({
  manualChunks: {
    "vendor-maplibre": ["maplibre-gl"],
  },
});
