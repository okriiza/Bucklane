<script setup lang="ts">
import { CircleAlert, TriangleAlert } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  toAppError,
  type AppError,
  type DeleteFailure,
  type PrefixDeletePlan,
  type StorageObject,
} from "@/services/tauri";
import { useExplorerStore } from "@/stores/explorer";
import { formatBytes } from "@/utils/format";

const props = defineProps<{ entries: StorageObject[] }>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ deleted: [] }>();

const explorer = useExplorerStore();
const { t } = useI18n();

const plans = ref<PrefixDeletePlan[]>([]);
const preparing = ref(false);
const deleting = ref(false);
const error = ref<AppError | null>(null);
const failures = ref<DeleteFailure[]>([]);

const files = computed(() => props.entries.filter((e) => !e.isPrefix));
const folders = computed(() => props.entries.filter((e) => e.isPrefix));
const totalObjects = computed(
  () => files.value.length + plans.value.reduce((n, p) => n + p.objectCount, 0),
);
const totalBytes = computed(
  () =>
    files.value.reduce((n, f) => n + (f.size ?? 0), 0) +
    plans.value.reduce((n, p) => n + (p.totalBytes ?? 0), 0),
);
const ready = computed(
  () => !preparing.value && !error.value && plans.value.length === folders.value.length,
);

// Folders are counted before confirmation so the user sees exactly how many
// objects will be removed (FIL-006, TSD §6.3.2).
watch(open, async (isOpen) => {
  if (!isOpen) return;
  plans.value = [];
  failures.value = [];
  error.value = null;
  if (!folders.value.length) return;

  preparing.value = true;
  try {
    plans.value = await Promise.all(folders.value.map((f) => explorer.prepareFolderDelete(f.key)));
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    preparing.value = false;
  }
});

async function confirm() {
  deleting.value = true;
  try {
    const result = await explorer.deleteEntries(
      files.value.map((f) => f.key),
      plans.value.map((p) => p.planId),
    );
    explorer.setAllChecked([], false);
    emit("deleted");
    if (result.failed.length) {
      // Never report full success on partial failure; keep the dialog open.
      failures.value = result.failed;
      plans.value = [];
    } else {
      toast.success(t("delete.done", { count: result.deleted.length }, result.deleted.length));
      open.value = false;
    }
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    deleting.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-lg">
      <template v-if="failures.length">
        <DialogTitle class="text-lg font-semibold">{{ $t("delete.partialTitle") }}</DialogTitle>
        <DialogDescription class="text-muted-foreground text-sm">
          {{ $t("delete.partialDescription", { count: failures.length }, failures.length) }}
        </DialogDescription>
        <ul class="max-h-60 overflow-auto rounded-md border text-xs">
          <li v-for="f in failures" :key="f.key" class="border-b px-3 py-2 last:border-b-0">
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
          {{ $t("delete.title", { count: entries.length }, entries.length) }}
        </DialogTitle>
        <DialogDescription class="text-muted-foreground text-sm">
          {{ $t("delete.fromBucket", { bucket: explorer.location?.bucket }) }}
        </DialogDescription>

        <p v-if="preparing" class="text-muted-foreground text-sm">{{ $t("delete.counting") }}</p>
        <p
          v-else-if="error"
          class="border-destructive/40 bg-destructive/10 text-destructive flex gap-2 rounded-md border p-3 text-sm"
          role="alert"
        >
          <CircleAlert class="mt-0.5 size-4 shrink-0" />
          {{ error.message }}
        </p>
        <template v-else>
          <p
            class="border-destructive/40 bg-destructive/10 text-destructive flex gap-2 rounded-md border p-3 text-sm"
          >
            <TriangleAlert class="mt-0.5 size-4 shrink-0" />
            <span>
              {{
                $t(
                  "delete.summary",
                  { count: totalObjects, size: formatBytes(totalBytes) },
                  totalObjects,
                )
              }}
            </span>
          </p>
          <ul class="max-h-48 overflow-auto rounded-md border text-xs">
            <li
              v-for="entry in entries"
              :key="entry.key"
              class="border-b px-3 py-1.5 font-mono break-all last:border-b-0"
            >
              {{ entry.key }}
              <span v-if="entry.isPrefix" class="text-muted-foreground font-sans">
                —
                {{
                  $t(
                    "delete.folderCount",
                    { count: plans.find((p) => p.prefix === entry.key)?.objectCount ?? 0 },
                    plans.find((p) => p.prefix === entry.key)?.objectCount ?? 0,
                  )
                }}
              </span>
            </li>
          </ul>
        </template>

        <div class="flex justify-end gap-2">
          <Button variant="outline" :disabled="deleting" @click="open = false">
            {{ $t("common.cancel") }}
          </Button>
          <Button variant="destructive" :disabled="!ready || deleting" @click="confirm">
            {{ deleting ? $t("delete.deleting") : $t("common.delete") }}
          </Button>
        </div>
      </template>
    </DialogContent>
  </Dialog>
</template>
