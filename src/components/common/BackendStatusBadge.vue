<script setup lang="ts">
import { computed, onMounted } from "vue";

import { useAppStore, type BackendStatus } from "@/stores/app";

const app = useAppStore();

const labels: Record<BackendStatus, string> = {
  checking: "status.checking",
  ready: "status.ready",
  "database-unavailable": "status.databaseUnavailable",
  unreachable: "status.backendUnreachable",
};

const dotClass = computed(() => {
  switch (app.status) {
    case "ready":
      return "bg-success";
    case "checking":
      return "bg-muted-foreground animate-pulse";
    default:
      return "bg-destructive";
  }
});

onMounted(() => app.checkHealth());
</script>

<template>
  <div class="text-muted-foreground flex items-center gap-2 text-xs" role="status">
    <span class="size-2 rounded-full" :class="dotClass" />
    <span>{{ $t(labels[app.status]) }}</span>
    <span v-if="app.health" class="ml-auto font-mono">
      {{ $t("status.version", { version: app.health.appVersion }) }}
    </span>
  </div>
</template>
