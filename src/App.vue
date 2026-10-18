<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";

import { useTheme } from "@/composables/useTheme";
import AppLayout from "@/layouts/AppLayout.vue";
import { useSettingsStore } from "@/stores/settings";
import { useUpdateStore } from "@/stores/updates";

const settings = useSettingsStore();
const updates = useUpdateStore();
const router = useRouter();
const { t } = useI18n();
useTheme();

/** One quiet check per launch; failures only show up under About. */
async function checkForUpdatesOnStartup() {
  // Dev builds have no published release to compare against.
  if (import.meta.env.DEV || !settings.settings.checkForUpdates) return;
  const update = await updates.check();
  if (!update) return;
  toast.info(t("updates.toast", { version: update.version }), {
    duration: 15_000,
    action: { label: t("updates.view"), onClick: () => void router.push("/about") },
  });
}

onMounted(async () => {
  await settings.load();
  void checkForUpdatesOnStartup();
});
</script>

<template>
  <AppLayout />
</template>
