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
      return "bg-success shadow-[0_0_0_3px] shadow-success/15";
    case "checking":
      return "bg-muted-foreground animate-pulse";
    default:
      return "bg-destructive shadow-[0_0_0_3px] shadow-destructive/15";
  }
});

onMounted(() => app.checkHealth());
</script>

<template>
  <div class="text-muted-foreground flex items-center gap-2 text-[11px]" role="status">
    <span class="size-1.5 rounded-full" :class="dotClass" />
    <span>{{ $t(labels[app.status]) }}</span>
    <span v-if="app.health" class="tabular ml-auto font-mono opacity-70">
      {{ $t("status.version", { version: app.health.appVersion }) }}
    </span>
  </div>
</template>
