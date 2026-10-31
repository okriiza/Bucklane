<script setup lang="ts">
import {
  ArrowDown,
  ArrowUp,
  ChevronDown,
  ChevronUp,
  FolderSearch,
  Play,
  RotateCw,
  X,
} from "@lucide/vue";
import { onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { toAppError, type TransferDto } from "@/services/tauri";
import { useTransferStore } from "@/stores/transfers";
import { formatBytes, formatDuration, formatSpeed } from "@/utils/format";

const store = useTransferStore();
const { t: translate } = useI18n();
/** The file manager has a different name on each platform. */
const revealLabel = translate(
  navigator.userAgent.includes("Mac")
    ? "transfers.revealMac"
    : navigator.userAgent.includes("Windows")
      ? "transfers.revealWindows"
      : "transfers.revealLinux",
);
// Collapsed while idle so it doesn't take space from the page. It opens when
// a transfer starts, and closes again shortly after a batch finishes cleanly.
// Failures keep it open, and a manual toggle always wins.
const AUTO_COLLAPSE_MS = 3000;
const collapsed = ref(store.activeCount === 0);
let userToggled = false;
let failedAtBatchStart = store.failedCount;
let collapseTimer: ReturnType<typeof setTimeout> | undefined;

watch(
  () => store.activeCount,
  (count, previous = 0) => {
    if (count > 0 && previous === 0) {
      failedAtBatchStart = store.failedCount;
      userToggled = false;
      clearTimeout(collapseTimer);
      collapsed.value = false;
    } else if (count === 0 && previous > 0 && store.failedCount <= failedAtBatchStart) {
      collapseTimer = setTimeout(() => {
        if (!userToggled && store.activeCount === 0) collapsed.value = true;
      }, AUTO_COLLAPSE_MS);
    }
  },
);

function toggle() {
  userToggled = true;
  clearTimeout(collapseTimer);
  collapsed.value = !collapsed.value;
}

onUnmounted(() => clearTimeout(collapseTimer));

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
  RUNNING: "text-primary",
  COMPLETED: "text-success",
  FAILED: "text-destructive",
  CANCELED: "text-muted-foreground",
};
</script>

<template>
  <section
    class="bg-sidebar/50 flex flex-col border-t transition-[height] duration-200"
    :class="collapsed ? 'h-9' : 'h-52'"
  >
    <header class="flex h-9 shrink-0 items-center gap-3 border-b px-4 text-xs">
      <button
        type="button"
        class="flex items-center gap-1.5 font-medium"
        :aria-expanded="!collapsed"
        @click="toggle"
      >
        <ChevronDown v-if="!collapsed" class="size-3.5" />
        <ChevronUp v-else class="size-3.5" />
        {{ $t("transfers.title") }}
      </button>
      <span
        v-if="store.activeCount"
        class="bg-primary/12 text-primary tabular rounded px-1.5 py-0.5 text-[11px] font-medium"
      >
        {{ $t("transfers.active", { count: store.activeCount }) }}
      </span>
      <span
        v-if="store.failedCount"
        class="bg-destructive/12 text-destructive tabular rounded px-1.5 py-0.5 text-[11px] font-medium"
      >
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
          class="hover:bg-accent/40 grid grid-cols-[1rem_minmax(0,1fr)_10rem_9rem_3.5rem] items-center gap-3 px-4 py-2 transition-colors"
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
              class="bg-muted h-1 overflow-hidden rounded-full"
              role="progressbar"
              :aria-valuenow="percent(t) ?? undefined"
              aria-valuemin="0"
              aria-valuemax="100"
            >
              <div
                class="h-full rounded-full transition-[width] duration-300"
                :class="
                  t.resumableBytes
                    ? 'bg-warning'
                    : t.status === 'FAILED'
                      ? 'bg-destructive'
                      : 'bg-primary'
                "
                :style="{ width: `${percent(t) ?? 0}%` }"
              />
            </div>
            <div class="text-muted-foreground tabular mt-1 font-mono text-[11px]">
              {{ formatBytes(t.bytesTransferred) }}
              <template v-if="t.bytesTotal != null">/ {{ formatBytes(t.bytesTotal) }}</template>
            </div>
          </div>
          <div class="min-w-0">
            <div :class="STATUS_CLASS[t.status]">
              {{ $t(`transfers.status.${t.status}`) }}
              <template v-if="t.status === 'RUNNING' && percent(t) != null">
                {{ percent(t) }}%
              </template>
            </div>
            <div
              v-if="t.resumableBytes"
              class="text-muted-foreground tabular truncate text-[11px]"
              :title="t.error?.message"
            >
              {{ $t("transfers.resumable", { size: formatBytes(t.resumableBytes) }) }}
            </div>
            <div
              v-else-if="t.status === 'RUNNING' && store.speedOf(t.id) != null"
              class="text-muted-foreground tabular truncate font-mono text-[11px]"
            >
              {{ formatSpeed(store.speedOf(t.id)) }}
              <template v-if="store.etaOf(t) != null">
                · {{ $t("transfers.eta", { time: formatDuration(store.etaOf(t)) }) }}
              </template>
            </div>
          </div>
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
              v-else-if="t.status === 'COMPLETED'"
              variant="ghost"
              size="icon"
              class="size-6"
              :title="revealLabel"
              :aria-label="revealLabel"
              @click="run(() => store.reveal(t.id))"
            >
              <FolderSearch />
            </Button>
            <Button
              v-else-if="t.status === 'FAILED' && t.connectionId && t.resumableBytes"
              variant="ghost"
              size="icon"
              class="text-primary size-6"
              :title="
                $t('transfers.resume', {
                  done: formatBytes(t.resumableBytes),
                  total: formatBytes(t.bytesTotal),
                })
              "
              :aria-label="
                $t('transfers.resume', {
                  done: formatBytes(t.resumableBytes),
                  total: formatBytes(t.bytesTotal),
                })
              "
              @click="run(() => store.retry(t.id))"
            >
              <Play class="fill-current" />
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
