import { createRouter, createWebHashHistory } from "vue-router";

import HomePage from "@/pages/HomePage.vue";

export const router = createRouter({
  // Hash history: the app is served from a custom protocol, not a web server.
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomePage },
    {
      path: "/connections/:id",
      name: "connection",
      component: () => import("@/pages/ConnectionPage.vue"),
      props: true,
    },
    {
      path: "/connections/:id/buckets/:bucket",
      name: "explorer",
      component: () => import("@/pages/ExplorerPage.vue"),
      // The prefix lives in the query string: it may contain any characters,
      // including "/", and must round-trip exactly.
      props: (route) => ({
        id: route.params.id,
        bucket: route.params.bucket,
        prefix: typeof route.query.prefix === "string" ? route.query.prefix : "",
      }),
    },
    { path: "/settings", name: "settings", component: () => import("@/pages/SettingsPage.vue") },
    { path: "/about", name: "about", component: () => import("@/pages/AboutPage.vue") },
  ],
});
