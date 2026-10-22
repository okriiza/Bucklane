<script setup lang="ts">
import { CircleAlert, TriangleAlert } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  events,
  toAppError,
  type AppError,
  type DeleteFailure,
  type ExistingObjectPolicy,
  type PrefixRelocatePlan,
} from "@/services/tauri";
import { useExplorerStore } from "@/stores/explorer";
import { formatBytes } from "@/utils/format";

export interface FolderRelocateRequest {
  kind: "rename" | "move" | "copy";
  source: string;
  destination: string;
}

const props = defineProps<{ request: FolderRelocateRequest | null }>();
const open = defineModel<boolean>("open", { required: true });

const explorer = useExplorerStore();
const { t } = useI18n();

const plan = ref<PrefixRelocatePlan | null>(null);
const preparing = ref(false);
const running = ref(false);
const canceling = ref(false);
const error = ref<AppError | null>(null);
const existing = ref<ExistingObjectPolicy>("SKIP");
const progress = ref({ done: 0, total: 0 });
const problems = ref<{ title: string; items: DeleteFailure[] } | null>(null);

const kind = computed(() => props.request?.kind ?? "move");
const percent = computed(() =>
  progress.value.total ? Math.round((progress.value.done / progress.value.total) * 100) : 0,
);

// Count the folder before anything changes, like folder delete (TSD §6.3.2).
watch(open, async (isOpen) => {
  if (!isOpen || !props.request) return;
  plan.value = null;
  error.value = null;
  problems.value = null;
  existing.value = "SKIP";
  progress.value = { done: 0, total: 0 };
  preparing.value = true;
  try {
    const { source, destination } = props.request;
    plan.value = await explorer.prepareFolderRelocate(
      source,
      destination,
      kind.value === "copy" ? "COPY" : "MOVE",
    );
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    preparing.value = false;
  }
});

/** A running copy/move cannot be dismissed; use Stop instead. */
function setOpen(value: boolean) {
  if (!running.value) open.value = value;
}

async function confirm() {
  const current = plan.value;
  if (!current) return;
  running.value = true;
  progress.value = { done: 0, total: current.objectCount };
  const unlisten = await events.folderRelocateProgress.listen(({ payload }) => {
    if (payload.planId === current.planId) progress.value = payload;
  });
  try {
    const result = await explorer.executeFolderRelocate(current.planId, existing.value);
    const failed = [...result.failed, ...result.sourcesKept];
    if (failed.length) {
      // Never report full success on partial failure; keep the dialog open.
      problems.value = {
        title: result.failed.length
          ? t("folderOps.failedTitle", { count: result.failed.length }, result.failed.length)
          : t(
              "folderOps.keptTitle",
              { count: result.sourcesKept.length },
              result.sourcesKept.length,
            ),
        items: failed,
      };
    } else {
      const parts = [t(`folderOps.${kind.value}Done`, { count: result.completed })];
      if (result.skipped) parts.push(t("folderOps.skipped", { count: result.skipped }));
      if (result.canceled) parts.push(t("folderOps.canceled", { count: result.canceled }));
      (result.canceled ? toast.warning : toast.success)(parts.join(" · "));
      running.value = false;
      open.value = false;
    }
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    unlisten();
    running.value = false;
    canceling.value = false;
  }
}

async function stop() {
  if (!plan.value) return;
  canceling.value = true;
  try {
    await explorer.cancelFolderRelocate(plan.value.planId);
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="setOpen">
    <DialogContent class="max-w-lg">
      <template v-if="problems">
        <DialogTitle class="text-lg font-semibold">{{ problems.title }}</DialogTitle>
        <DialogDescription class="text-muted-foreground text-sm">
          {{ $t("folderOps.problemsHint") }}
        </DialogDescription>
        <ul class="max-h-60 overflow-auto rounded-md border text-xs">
          <li v-for="f in problems.items" :key="f.key" class="border-b px-3 py-2 last:border-b-0">
            <div class="font-mono break-all select-text">{{ f.key }}</div>
            <div class="text-destructive">{{ f.error.message }}</div>
          </li>
        </ul>
        <div class="flex justify-end">
          <Button @click="open = false">{{ $t("common.close") }}</Button>
        </div>
      </template>

      <template v-else>
        <DialogTitle class="text-lg font-semibold">
          {{ $t(`folderOps.${kind}Title`) }}
        </DialogTitle>
        <DialogDescription class="text-muted-foreground text-sm break-all">
          <span class="font-mono">{{ request?.source }}</span>
          →
          <span class="text-foreground font-mono">{{ request?.destination }}</span>
        </DialogDescription>

        <p v-if="preparing" class="text-muted-foreground text-sm">
          {{ $t("folderOps.counting") }}
        </p>
        <p
          v-else-if="error"
          class="border-destructive/40 bg-destructive/10 text-destructive flex gap-2 rounded-md border p-3 text-sm"
          role="alert"
        >
          <CircleAlert class="mt-0.5 size-4 shrink-0" />
          {{ error.message }}
        </p>

        <template v-else-if="plan && running">
          <div
            class="bg-muted h-2 overflow-hidden rounded-full"
            role="progressbar"
            :aria-valuenow="percent"
            aria-valuemin="0"
            aria-valuemax="100"
          >
            <div
              class="bg-primary h-full rounded-full transition-[width]"
              :style="{ width: `${percent}%` }"
            />
          </div>
          <p class="text-muted-foreground tabular text-sm">
            {{ $t("folderOps.progress", { done: progress.done, total: progress.total }) }}
          </p>
        </template>

        <template v-else-if="plan">
          <p class="text-sm">
            {{
              $t(
                "folderOps.summary",
                { count: plan.objectCount, size: formatBytes(plan.totalBytes) },
                plan.objectCount,
              )
            }}
          </p>

          <div
            v-if="plan.conflictCount"
            class="border-warning/40 bg-warning/10 grid gap-2 rounded-md border p-3 text-sm"
          >
            <p class="flex gap-2">
              <TriangleAlert class="text-warning mt-0.5 size-4 shrink-0" />
              <span>
                {{ $t("folderOps.conflicts", { count: plan.conflictCount }, plan.conflictCount) }}
              </span>
            </p>
            <ul class="text-muted-foreground max-h-24 overflow-auto pl-6 font-mono text-xs">
              <li v-for="key in plan.conflictSamples" :key="key" class="break-all">{{ key }}</li>
              <li v-if="plan.conflictCount > plan.conflictSamples.length">…</li>
            </ul>
            <div class="grid gap-1.5 pl-6">
              <label class="flex items-center gap-2">
                <input v-model="existing" type="radio" value="SKIP" class="accent-primary" />
                {{ $t("folderOps.skipExisting") }}
              </label>
              <label class="flex items-center gap-2">
                <input v-model="existing" type="radio" value="OVERWRITE" class="accent-primary" />
                {{ $t("folderOps.replaceExisting") }}
              </label>
            </div>
          </div>

          <p v-if="kind !== 'copy'" class="text-muted-foreground text-xs">
            {{ $t("folderOps.moveSafety") }}
          </p>
        </template>

        <div class="flex justify-end gap-2">
          <template v-if="running">
            <Button variant="outline" :disabled="canceling" @click="stop">
              {{ canceling ? $t("folderOps.stopping") : $t("folderOps.stop") }}
            </Button>
          </template>
          <template v-else>
            <Button variant="outline" @click="open = false">{{ $t("common.cancel") }}</Button>
            <Button :disabled="!plan || preparing || !!error" @click="confirm">
              {{ $t(`ops.${kind}`) }}
            </Button>
          </template>
        </div>
      </template>
    </DialogContent>
  </Dialog>
</template>
