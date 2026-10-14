<script setup lang="ts">
import { Upload } from "@lucide/vue";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { toast } from "vue-sonner";

import { useTransferActions } from "@/composables/useTransferActions";
import { events } from "@/services/tauri";
import { useExplorerStore } from "@/stores/explorer";

const route = useRoute();
const explorer = useExplorerStore();
const { uploadSelections } = useTransferActions();
const { t } = useI18n();

const hovering = ref(false);
/** Uploads go to the folder open in the explorer, and only there. */
const target = computed(() => (route.name === "explorer" ? explorer.location : null));

const unlisteners: (() => void)[] = [];

onMounted(async () => {
  unlisteners.push(await events.fileDragHover.listen((e) => (hovering.value = e.payload)));
  unlisteners.push(
    await events.filesDropped.listen(async ({ payload }) => {
      hovering.value = false;
      if (payload.skipped) {
        toast.info(t("drop.skippedFolders", { count: payload.skipped }, payload.skipped));
      }
      if (!payload.files.length) return;
      if (!target.value) {
        toast.info(t("drop.openFolderFirst"));
        return;
      }
      await uploadSelections(target.value, payload.files);
    }),
  );
});

onUnmounted(() => unlisteners.forEach((stop) => stop()));
</script>

<template>
  <div
    v-if="hovering"
    class="bg-background/80 pointer-events-none fixed inset-0 z-40 flex items-center justify-center p-8 backdrop-blur-sm"
  >
    <div
      class="flex max-w-md flex-col items-center gap-3 rounded-xl border-2 border-dashed p-10 text-center"
      :class="target ? 'border-primary' : 'border-muted-foreground/40'"
    >
      <Upload class="size-8" :class="target ? 'text-primary' : 'text-muted-foreground'" />
      <template v-if="target">
        <p class="font-medium">{{ $t("drop.title") }}</p>
        <p class="text-muted-foreground font-mono text-sm break-all">
          {{ target.bucket }}/{{ target.prefix }}
        </p>
      </template>
      <p v-else class="text-muted-foreground text-sm">{{ $t("drop.openFolderFirst") }}</p>
    </div>
  </div>
</template>
