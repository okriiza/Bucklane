<script setup lang="ts">
import BackendStatusBadge from "./BackendStatusBadge.vue";
</script>

<template>
  <aside class="bg-surface-muted border-border flex flex-col border-r">
    <div class="px-4 pt-5 pb-3 text-sm font-semibold tracking-tight">
      {{ $t("app.name") }}
    </div>

    <nav class="flex-1 overflow-auto px-2">
      <div class="text-fg-muted px-2 pt-2 pb-1 text-xs font-medium uppercase tracking-wide">
        {{ $t("nav.connections") }}
      </div>
      <p class="text-fg-muted px-2 py-1 text-sm">{{ $t("nav.noConnections") }}</p>
    </nav>

    <div class="border-border flex flex-col gap-0.5 border-t p-2">
      <RouterLink
        v-for="link in [
          { to: '/settings', label: 'nav.settings' },
          { to: '/about', label: 'nav.about' },
        ]"
        :key="link.to"
        :to="link.to"
        class="hover:bg-border/60 rounded-md px-2 py-1.5 text-sm"
        active-class="bg-border/60 font-medium"
      >
        {{ $t(link.label) }}
      </RouterLink>
      <BackendStatusBadge class="px-2 pt-2" />
    </div>
  </aside>
</template>
