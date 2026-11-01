<script setup lang="ts">
import { CircleAlert, CircleCheck, Download, ExternalLink, RefreshCw, RotateCw } from "@lucide/vue";
import { computed, ref } from "vue";
import { toast } from "vue-sonner";

import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import ReleaseNotes from "@/components/common/ReleaseNotes.vue";
import { Button } from "@/components/ui/button";
import { toAppError } from "@/services/tauri";
import { useTransferStore } from "@/stores/transfers";
import { useUpdateStore } from "@/stores/updates";
import { parseNotes } from "@/utils/changelog";
import { formatBytes } from "@/utils/format";

defineProps<{ currentVersion: string | null }>();

const updates = useUpdateStore();
const transfers = useTransferStore();

const confirmOpen = ref(false);
const pending = ref<"install" | "restart">("install");

/** Notes of the new version; older manifests only carry a link. */
const notes = computed(() => {
  const sections = parseNotes(updates.update?.notes ?? "");
  return sections.some((s) => s.title) ? sections : [];
});

const percent = computed(() =>
  updates.total ? Math.min(100, Math.round((updates.downloaded / updates.total) * 100)) : null,
);

/** Installing or restarting interrupts running transfers, so ask first. */
function guard(action: "install" | "restart") {
  if (transfers.activeCount > 0) {
    pending.value = action;
    confirmOpen.value = true;
  } else {
    void run(action);
  }
}

function run(action: "install" | "restart") {
  return action === "install" ? updates.install() : updates.restart();
}

async function openNotes(version: string) {
  try {
    await updates.openReleasePage(version);
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <section class="bg-card grid gap-3 rounded-xl border p-5 text-[13px] shadow-[var(--shadow-soft)]">
    <h2 class="text-muted-foreground text-[11px] font-medium">{{ $t("updates.title") }}</h2>

    <template v-if="updates.status === 'available' && updates.update">
      <p class="font-medium">
        {{ $t("updates.available", { version: updates.update.version }) }}
      </p>
      <div v-if="notes.length" class="bg-muted/40 max-h-56 overflow-auto rounded-lg border p-3">
        <ReleaseNotes :sections="notes" />
      </div>
      <div class="flex flex-wrap gap-2">
        <Button size="sm" @click="guard('install')">
          <Download /> {{ $t("updates.install") }}
        </Button>
        <Button variant="ghost" size="sm" @click="openNotes(updates.update.version)">
          <ExternalLink /> {{ $t("updates.whatsNew") }}
        </Button>
      </div>
    </template>

    <template v-else-if="updates.status === 'installing'">
      <p>{{ $t("updates.downloading") }}</p>
      <div
        class="bg-muted h-1.5 overflow-hidden rounded-full"
        role="progressbar"
        :aria-valuenow="percent ?? undefined"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <div
          class="bg-primary h-full rounded-full transition-[width]"
          :class="{ 'w-1/3 animate-pulse': percent === null }"
          :style="percent === null ? undefined : { width: `${percent}%` }"
        />
      </div>
      <p class="text-muted-foreground tabular text-xs">
        {{ formatBytes(updates.downloaded) }}
        <template v-if="updates.total"> / {{ formatBytes(updates.total) }}</template>
      </p>
    </template>

    <template v-else-if="updates.status === 'ready'">
      <p class="flex gap-2">
        <CircleCheck class="text-success mt-0.5 size-4 shrink-0" />
        {{ $t("updates.ready") }}
      </p>
      <Button size="sm" class="justify-self-start" @click="guard('restart')">
        <RotateCw /> {{ $t("updates.restart") }}
      </Button>
    </template>

    <template v-else>
      <p v-if="updates.status === 'upToDate'" class="flex gap-2">
        <CircleCheck class="text-success mt-0.5 size-4 shrink-0" />
        {{ $t("updates.upToDate", { version: currentVersion ?? "—" }) }}
      </p>
      <p
        v-else-if="updates.status === 'error' && updates.error"
        class="text-destructive flex gap-2"
      >
        <CircleAlert class="mt-0.5 size-4 shrink-0" />
        {{ updates.error.message }}
      </p>
      <p v-else class="text-muted-foreground">{{ $t("updates.hint") }}</p>
      <Button
        variant="outline"
        size="sm"
        class="justify-self-start"
        :disabled="updates.status === 'checking'"
        @click="updates.check()"
      >
        <RefreshCw :class="{ 'animate-spin': updates.status === 'checking' }" />
        {{ updates.status === "checking" ? $t("updates.checking") : $t("updates.check") }}
      </Button>
    </template>

    <ConfirmDialog
      v-model:open="confirmOpen"
      :title="$t('updates.transfersTitle')"
      :description="
        $t('updates.transfersDescription', { count: transfers.activeCount }, transfers.activeCount)
      "
      :confirm-label="pending === 'install' ? $t('updates.install') : $t('updates.restart')"
      destructive
      @confirm="run(pending)"
    />
  </section>
</template>
