import { createPinia } from "pinia";
import { createApp } from "vue";

import App from "./App.vue";
import { i18n } from "./i18n";
import { router } from "./router";
import "./assets/main.css";

// macOS draws an overlay title bar; the layout reserves room for it.
document.documentElement.classList.toggle("platform-mac", navigator.userAgent.includes("Mac"));

// Screenshot demo data (`?demo`); stripped from production builds.
const demo =
  import.meta.env.DEV && new URLSearchParams(location.search).has("demo")
    ? await import("./demo/demoIpc")
    : null;
demo?.install();

const app = createApp(App).use(createPinia()).use(router).use(i18n);
app.mount("#app");
void demo?.afterMount(app);
