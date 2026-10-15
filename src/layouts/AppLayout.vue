<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useRoute } from "vue-router";
import { Toaster } from "vue-sonner";
import "vue-sonner/style.css";

import AppSidebar from "@/components/common/AppSidebar.vue";
import ConnectionFormDialog from "@/components/connections/ConnectionFormDialog.vue";
import ConflictDialog from "@/components/transfers/ConflictDialog.vue";
import FileDropOverlay from "@/components/transfers/FileDropOverlay.vue";
import TransferPanel from "@/components/transfers/TransferPanel.vue";
import { useExplorerStore } from "@/stores/explorer";
import { useSettingsStore } from "@/stores/settings";
import { useTransferStore } from "@/stores/transfers";

const transfers = useTransferStore();
const explorer = useExplorerStore();
const settings = useSettingsStore();
const route = useRoute();

// Refresh the open folder once uploads into it complete. Bursts of
// completions (many small files) are coalesced into one listing request.
let refreshTimer: ReturnType<typeof setTimeout> | undefined;
const stopListening = transfers.onCompleted((t) => {
  const loc = explorer.location;
  if (t.direction !== "UPLOAD" || !loc || t.connectionId !== loc.connectionId) return;
  if (t.bucket !== loc.bucket || !t.objectKey.startsWith(loc.prefix)) return;
  if (t.objectKey.slice(loc.prefix.length).includes("/")) return;
  clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => explorer.refresh(), 500);
});

onMounted(() => transfers.start());
onUnmounted(() => {
  stopListening();
  clearTimeout(refreshTimer);
});
</script>

<!-- Three-area layout from PRD §7.1: sidebar, explorer, bottom transfer panel. -->
<template>
  <div class="flex h-full">
    <AppSidebar class="w-64 shrink-0" />
    <div class="flex min-w-0 flex-1 flex-col">
      <!-- Title-bar drag area on macOS; the explorer header provides its own. -->
      <div
        v-if="route.name !== 'explorer'"
        data-tauri-drag-region
        class="mac:block hidden h-8 shrink-0"
      />
      <main class="min-h-0 flex-1 overflow-auto">
        <RouterView />
      </main>
      <TransferPanel class="shrink-0" />
    </div>
  </div>
  <ConnectionFormDialog />
  <ConflictDialog />
  <FileDropOverlay />
  <Toaster
    position="bottom-right"
    rich-colors
    close-button
    :theme="settings.settings.theme"
    :toast-options="{ class: 'font-sans text-[13px]' }"
  />
</template>
