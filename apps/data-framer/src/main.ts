import { createApp } from "vue";
import "./assets/shared.css";
import App from "./App.vue";
import { ModuleRegistry, AllCommunityModule } from "ag-charts-community";

ModuleRegistry.registerModules([AllCommunityModule]);

createApp(App).mount("#app");
