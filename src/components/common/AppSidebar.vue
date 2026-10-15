<script setup lang="ts">
import ConnectionList from "@/components/connections/ConnectionList.vue";

import BackendStatusBadge from "./BackendStatusBadge.vue";
</script>

<template>
  <aside class="bg-sidebar text-sidebar-foreground border-sidebar-border flex flex-col border-r">
    <div class="px-4 pt-5 pb-3 text-sm font-semibold tracking-tight">
      {{ $t("app.name") }}
    </div>

    <nav class="min-h-0 flex-1 overflow-auto px-2">
      <ConnectionList />
    </nav>

    <div class="border-sidebar-border flex flex-col gap-0.5 border-t p-2">
      <RouterLink
        v-for="link in [
          { to: '/settings', label: 'nav.settings' },
          { to: '/about', label: 'nav.about' },
        ]"
        :key="link.to"
        :to="link.to"
        class="hover:bg-sidebar-accent rounded-md px-2 py-1.5 text-sm"
        active-class="bg-sidebar-accent font-medium"
      >
        {{ $t(link.label) }}
      </RouterLink>
      <BackendStatusBadge class="px-2 pt-2" />
    </div>
  </aside>
</template>
