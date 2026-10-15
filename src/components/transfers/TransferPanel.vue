<script setup lang="ts">
import { ArrowDown, ArrowUp, ChevronDown, ChevronUp, RotateCw, X } from "@lucide/vue";
import { ref } from "vue";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { toAppError, type TransferDto } from "@/services/tauri";
import { useTransferStore } from "@/stores/transfers";
import { formatBytes } from "@/utils/format";

const store = useTransferStore();
const collapsed = ref(false);

function percent(t: TransferDto) {
  if (t.status === "COMPLETED") return 100;
  if (!t.bytesTotal) return null;
  return Math.min(100, Math.floor(((t.bytesTransferred ?? 0) / t.bytesTotal) * 100));
}

function name(t: TransferDto) {
  return t.direction === "UPLOAD" ? t.localFileName : (t.objectKey.split("/").pop() ?? t.objectKey);
}

async function run(action: () => Promise<void>) {
  try {
    await action();
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}

const STATUS_CLASS: Record<TransferDto["status"], string> = {
  QUEUED: "text-muted-foreground",
  RUNNING: "text-sky-600 dark:text-sky-400",
  COMPLETED: "text-success",
  FAILED: "text-destructive",
  CANCELED: "text-muted-foreground",
};
</script>

<template>
  <section class="flex flex-col border-t" :class="collapsed ? 'h-9' : 'h-48'">
    <header class="flex h-9 shrink-0 items-center gap-3 border-b px-4 text-xs">
      <button
        type="button"
        class="flex items-center gap-1.5 font-medium"
        :aria-expanded="!collapsed"
        @click="collapsed = !collapsed"
      >
        <ChevronDown v-if="!collapsed" class="size-3.5" />
        <ChevronUp v-else class="size-3.5" />
        {{ $t("transfers.title") }}
      </button>
      <span v-if="store.activeCount" class="text-muted-foreground">
        {{ $t("transfers.active", { count: store.activeCount }) }}
      </span>
      <span v-if="store.failedCount" class="text-destructive">
        {{ $t("transfers.failed", { count: store.failedCount }) }}
      </span>
      <Button
        v-if="store.hasFinished && !collapsed"
        variant="ghost"
        size="sm"
        class="ml-auto h-6 text-xs"
        @click="run(store.clearFinished)"
      >
        {{ $t("transfers.clearFinished") }}
      </Button>
    </header>

    <div v-if="!collapsed" class="min-h-0 flex-1 overflow-auto">
      <p v-if="store.loadError" class="text-destructive p-4 text-sm">
        {{ store.loadError.message }}
      </p>
      <p
        v-else-if="!store.transfers.length"
        class="text-muted-foreground flex h-full items-center justify-center text-sm"
      >
        {{ $t("transfers.empty") }}
      </p>
      <ul v-else class="divide-y text-xs">
        <li
          v-for="t in store.transfers"
          :key="t.id"
          class="grid grid-cols-[1rem_minmax(0,1fr)_10rem_7rem_3.5rem] items-center gap-3 px-4 py-1.5"
        >
          <component
            :is="t.direction === 'UPLOAD' ? ArrowUp : ArrowDown"
            class="text-muted-foreground size-3.5"
            :aria-label="
              t.direction === 'UPLOAD' ? $t('transfers.upload') : $t('transfers.download')
            "
          />
          <div class="min-w-0">
            <div class="truncate font-medium" :title="t.objectKey">{{ name(t) }}</div>
            <div class="text-muted-foreground truncate">
              <template v-if="t.error && t.status !== 'CANCELED'">
                <span class="text-destructive" :title="t.error.message">{{ t.error.message }}</span>
              </template>
              <template v-else>
                {{ t.connectionNameSnapshot }} · {{ t.bucket }}/{{ t.objectKey }}
              </template>
            </div>
          </div>
          <div>
            <div
              class="bg-muted h-1.5 overflow-hidden rounded-full"
              role="progressbar"
              :aria-valuenow="percent(t) ?? undefined"
              aria-valuemin="0"
              aria-valuemax="100"
            >
              <div
                class="h-full rounded-full transition-[width] duration-300"
                :class="t.status === 'FAILED' ? 'bg-destructive' : 'bg-primary'"
                :style="{ width: `${percent(t) ?? 0}%` }"
              />
            </div>
            <div class="text-muted-foreground mt-0.5 tabular-nums">
              {{ formatBytes(t.bytesTransferred) }}
              <template v-if="t.bytesTotal != null">/ {{ formatBytes(t.bytesTotal) }}</template>
            </div>
          </div>
          <span :class="STATUS_CLASS[t.status]">
            {{ $t(`transfers.status.${t.status}`) }}
            <template v-if="t.status === 'RUNNING' && percent(t) != null">
              {{ percent(t) }}%
            </template>
          </span>
          <div class="flex justify-end">
            <Button
              v-if="t.status === 'QUEUED' || t.status === 'RUNNING'"
              variant="ghost"
              size="icon"
              class="size-6"
              :title="$t('transfers.cancel')"
              :aria-label="$t('transfers.cancel')"
              @click="run(() => store.cancel(t.id))"
            >
              <X />
            </Button>
            <Button
              v-else-if="t.status === 'FAILED' && t.connectionId"
              variant="ghost"
              size="icon"
              class="size-6"
              :title="$t('transfers.retry')"
              :aria-label="$t('transfers.retry')"
              @click="run(() => store.retry(t.id))"
            >
              <RotateCw />
            </Button>
          </div>
        </li>
      </ul>
    </div>
  </section>
</template>
