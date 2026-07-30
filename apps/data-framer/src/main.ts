import "@viewers/ui/theme.css";
import { createApp } from "vue";
import App from "./App.vue";
import { ModuleRegistry, AllCommunityModule } from "ag-charts-community";
import "./assets/shared.css";

ModuleRegistry.registerModules([AllCommunityModule]);

createApp(App).mount("#app");
