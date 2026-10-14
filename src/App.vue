<script setup lang="ts">
import { useSystemTheme } from "@/composables/useSystemTheme";
import AppLayout from "@/layouts/AppLayout.vue";

useSystemTheme();
</script>

<template>
  <AppLayout />
</template>
