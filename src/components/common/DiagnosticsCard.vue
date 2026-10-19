<script setup lang="ts">
import { ClipboardCopy, FolderOpen } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { commands, toAppError, unwrap } from "@/services/tauri";
import { useConnectionStore } from "@/stores/connections";
import { useSettingsStore } from "@/stores/settings";
import { useTransferStore } from "@/stores/transfers";
import { copyText } from "@/utils/clipboard";
import { formatDiagnostics } from "@/utils/diagnostics";

const { t } = useI18n();
const connections = useConnectionStore();
const settings = useSettingsStore();
const transfers = useTransferStore();

async function copyDiagnostics() {
  try {
    const info = await unwrap(commands.diagnostics());
    const transfersByStatus: Record<string, number> = {};
    for (const tr of transfers.transfers) {
      transfersByStatus[tr.status] = (transfersByStatus[tr.status] ?? 0) + 1;
    }
    await copyText(
      formatDiagnostics({
        info,
        settings: settings.settings,
        providers: connections.connections.map((c) => c.provider),
        transfersByStatus,
      }),
    );
    toast.success(t("diagnostics.copied"));
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}

async function openLogs() {
  try {
    await unwrap(commands.openLogsFolder());
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <section class="bg-card grid gap-3 rounded-xl border p-5 text-[13px] shadow-[var(--shadow-soft)]">
    <h2 class="text-muted-foreground text-[11px] font-medium">{{ $t("diagnostics.title") }}</h2>
    <p class="text-muted-foreground">{{ $t("diagnostics.hint") }}</p>
    <div class="flex flex-wrap gap-2">
      <Button variant="outline" size="sm" @click="copyDiagnostics">
        <ClipboardCopy /> {{ $t("diagnostics.copy") }}
      </Button>
      <Button variant="ghost" size="sm" @click="openLogs">
        <FolderOpen /> {{ $t("diagnostics.openLogs") }}
      </Button>
    </div>
  </section>
</template>
