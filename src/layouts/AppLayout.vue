<script setup lang="ts">
import { Toaster } from "vue-sonner";
import "vue-sonner/style.css";

import AppSidebar from "@/components/common/AppSidebar.vue";
import ConnectionFormDialog from "@/components/connections/ConnectionFormDialog.vue";
import TransferPanel from "@/components/transfers/TransferPanel.vue";
</script>

<!-- Three-area layout from PRD §7.1: sidebar, explorer, bottom transfer panel. -->
<template>
  <div class="flex h-full">
    <AppSidebar class="w-60 shrink-0" />
    <div class="flex min-w-0 flex-1 flex-col">
      <main class="min-h-0 flex-1 overflow-auto">
        <RouterView />
      </main>
      <TransferPanel class="h-40 shrink-0" />
    </div>
  </div>
  <ConnectionFormDialog />
  <Toaster position="bottom-right" rich-colors />
</template>
