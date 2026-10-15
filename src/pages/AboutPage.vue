<script setup lang="ts">
import { useAppStore } from "@/stores/app";

const app = useAppStore();
</script>

<template>
  <div class="p-8">
    <h1 class="text-lg font-semibold">{{ $t("about.title") }}</h1>
    <p class="text-muted-foreground mt-2 text-sm">{{ $t("about.description") }}</p>
    <p v-if="app.health" class="text-muted-foreground mt-4 font-mono text-xs">
      {{ $t("status.version", { version: app.health.appVersion }) }}
    </p>
  </div>
</template>
