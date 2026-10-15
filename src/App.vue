<script setup lang="ts">
import { onMounted } from "vue";

import { useTheme } from "@/composables/useTheme";
import AppLayout from "@/layouts/AppLayout.vue";
import { useSettingsStore } from "@/stores/settings";

const settings = useSettingsStore();
useTheme();
onMounted(() => settings.load());
</script>

<template>
  <AppLayout />
</template>
