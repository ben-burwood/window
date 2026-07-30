import "@window/ui/theme.css";
import { createApp } from "vue";
import App from "./App.vue";
import "./assets/shared.css";

// PDF.js (>=4) uses Promise.withResolvers, which some older webviews lack.
// Polyfill it defensively so rendering works across all target platforms.
if (typeof (Promise as any).withResolvers !== "function") {
  (Promise as any).withResolvers = function () {
    let resolve!: (value: unknown) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    return { promise, resolve, reject };
  };
}

createApp(App).mount("#app");
