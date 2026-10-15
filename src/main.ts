import { createPinia } from "pinia";
import { createApp } from "vue";

import App from "./App.vue";
import { i18n } from "./i18n";
import { router } from "./router";
import "./assets/main.css";

// macOS draws an overlay title bar; the layout reserves room for it.
document.documentElement.classList.toggle("platform-mac", navigator.userAgent.includes("Mac"));

createApp(App).use(createPinia()).use(router).use(i18n).mount("#app");
