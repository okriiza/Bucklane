<script setup lang="ts">
import { onMounted, onUnmounted, watch } from "vue";
import { useRoute } from "vue-router";
import { Toaster } from "vue-sonner";
import "vue-sonner/style.css";

import AppSidebar from "@/components/common/AppSidebar.vue";
import ErrorBoundary from "@/components/common/ErrorBoundary.vue";
import ConnectionFormDialog from "@/components/connections/ConnectionFormDialog.vue";
import ConflictDialog from "@/components/transfers/ConflictDialog.vue";
import FileDropOverlay from "@/components/transfers/FileDropOverlay.vue";
import TransferPanel from "@/components/transfers/TransferPanel.vue";
import WhatsNewDialog from "@/components/common/WhatsNewDialog.vue";
import { useAppStore } from "@/stores/app";
import { useConnectionStore } from "@/stores/connections";
import { useExplorerStore } from "@/stores/explorer";
import { useSettingsStore } from "@/stores/settings";
import { useTransferStore } from "@/stores/transfers";
import { useWhatsNewStore } from "@/stores/whatsNew";
import { uploadChangesListing } from "@/utils/prefix";

const transfers = useTransferStore();
const explorer = useExplorerStore();
const settings = useSettingsStore();
const route = useRoute();
const app = useAppStore();
const whatsNew = useWhatsNewStore();
const connections = useConnectionStore();

// Saved connections tell an update apart from a fresh install.
watch(
  () => [app.health?.appVersion, connections.loaded] as const,
  ([version, loaded]) => {
    if (version && loaded) whatsNew.checkAfterUpdate(version, connections.connections.length > 0);
  },
  { immediate: true },
);

// Refresh the open folder once uploads, copies, or moves change it. Bursts of
// completions (many small files) are coalesced into one listing request.
let refreshTimer: ReturnType<typeof setTimeout> | undefined;
const stopListening = transfers.onCompleted((t) => {
  const loc = explorer.location;
  if (!loc) return;
  // A move also removes its original from the source folder.
  const from = t.source?.deleteAfterCopy ? t.source : null;
  const source =
    !!from &&
    from.connectionId === loc.connectionId &&
    from.bucket === loc.bucket &&
    from.key.startsWith(loc.prefix);
  const listed = new Set(explorer.entries.map((e) => e.key));
  const destination =
    t.direction !== "DOWNLOAD" &&
    t.connectionId === loc.connectionId &&
    t.bucket === loc.bucket &&
    uploadChangesListing(loc.prefix, t.objectKey, listed);
  if (!source && !destination) return;
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
        <ErrorBoundary>
          <RouterView />
        </ErrorBoundary>
      </main>
      <TransferPanel class="shrink-0" />
    </div>
  </div>
  <ConnectionFormDialog />
  <ConflictDialog />
  <WhatsNewDialog />
  <FileDropOverlay />
  <Toaster
    position="bottom-right"
    rich-colors
    close-button
    :theme="settings.settings.theme"
    :toast-options="{ class: 'font-sans text-[13px]' }"
  />
</template>
