import { createRouter, createWebHashHistory } from "vue-router";

import WelcomePage from "@/pages/WelcomePage.vue";

export const router = createRouter({
  // Hash history: the app is served from a custom protocol, not a web server.
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "welcome", component: WelcomePage },
    { path: "/settings", name: "settings", component: () => import("@/pages/SettingsPage.vue") },
    { path: "/about", name: "about", component: () => import("@/pages/AboutPage.vue") },
  ],
});
