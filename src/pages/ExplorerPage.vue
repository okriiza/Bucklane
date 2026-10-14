<script setup lang="ts">
import {
  ArrowUp,
  CircleAlert,
  Copy,
  FolderInput,
  FolderPlus,
  Pencil,
  RefreshCw,
  Search,
  Trash2,
  X,
} from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";

import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import PromptDialog from "@/components/common/PromptDialog.vue";
import DeleteObjectsDialog from "@/components/explorer/DeleteObjectsDialog.vue";
import ExplorerBreadcrumb from "@/components/explorer/ExplorerBreadcrumb.vue";
import ObjectDetails from "@/components/explorer/ObjectDetails.vue";
import ObjectTable from "@/components/explorer/ObjectTable.vue";
import { Button } from "@/components/ui/button";
import { CommandError, type ObjectOperationOutcome, type StorageObject } from "@/services/tauri";
import { useObjectDrag } from "@/composables/useObjectDrag";
import { toAppError } from "@/services/tauri";
import { useConnectionStore } from "@/stores/connections";
import { useExplorerStore } from "@/stores/explorer";
import { formatBytes } from "@/utils/format";
import { extensionOf, parentPrefix } from "@/utils/prefix";

const props = defineProps<{ id: string; bucket: string; prefix: string }>();

const router = useRouter();
const { t } = useI18n();
const connections = useConnectionStore();
const explorer = useExplorerStore();
const filter = ref("");

const connectionName = computed(() => connections.byId(props.id)?.name ?? "…");

watch(
  () => [props.id, props.bucket, props.prefix] as const,
  ([connectionId, bucket, prefix]) => {
    filter.value = "";
    explorer.open({ connectionId, bucket, prefix });
  },
  { immediate: true },
);

// Client-side filter over loaded entries only (EXP-004); it does not search
// the whole bucket.
const visible = computed(() => {
  const q = filter.value.trim().toLowerCase();
  if (!q) return explorer.entries;
  return explorer.entries.filter((e) => e.name.toLowerCase().includes(q));
});

type Relocation = "copy" | "move" | "rename";
type PromptKind = "newFolder" | Relocation;

const promptOpen = ref(false);
const prompt = ref<{ kind: PromptKind; entry?: StorageObject }>({ kind: "newFolder" });
const conflict = ref<{
  mode: Relocation;
  source: string;
  destination: string;
  size: number | null;
} | null>(null);
const conflictOpen = ref(false);
const deleteOpen = ref(false);
const deleteTargets = ref<StorageObject[]>([]);

const promptConfig = computed(() => {
  const entry = prompt.value.entry;
  switch (prompt.value.kind) {
    case "newFolder":
      return {
        title: t("ops.newFolderTitle"),
        label: t("ops.folderName"),
        initialValue: "",
        confirmLabel: t("ops.create"),
        selectRange: undefined,
      };
    case "rename": {
      const name = entry?.name ?? "";
      const ext = extensionOf(name);
      return {
        title: t("ops.renameTitle"),
        label: t("ops.newName"),
        initialValue: name,
        confirmLabel: t("ops.rename"),
        // Select the base name so typing keeps the extension.
        selectRange: [0, ext ? name.length - ext.length - 1 : name.length] as [number, number],
      };
    }
    default:
      return {
        title: prompt.value.kind === "copy" ? t("ops.copyTitle") : t("ops.moveTitle"),
        label: t("ops.destination"),
        initialValue: entry?.key ?? "",
        confirmLabel: prompt.value.kind === "copy" ? t("ops.copy") : t("ops.move"),
        selectRange: undefined,
      };
  }
});

function openPrompt(kind: PromptKind, entry?: StorageObject) {
  prompt.value = { kind, entry };
  promptOpen.value = true;
}

function invalidInput(message: string): never {
  throw new CommandError({ code: "INVALID_INPUT", message, details: null, retryable: false });
}

async function submitPrompt(value: string) {
  const { kind, entry } = prompt.value;
  if (kind === "newFolder") {
    await explorer.createFolder(value);
    toast.success(t("ops.folderCreated"));
    return;
  }
  if (!entry) return;
  let destination = value.trim();
  if (kind === "rename") {
    if (destination.includes("/")) invalidInput(t("ops.nameNoSlash"));
    destination = props.prefix + destination;
  }
  await relocate(kind, entry.key, destination, false);
}

async function relocate(mode: Relocation, source: string, destination: string, overwrite: boolean) {
  const outcome = await explorer.relocate(
    mode === "copy" ? "copy" : "move",
    source,
    destination,
    overwrite ? "OVERWRITE" : "ASK",
  );
  handleOutcome(mode, source, destination, outcome);
}

function handleOutcome(
  mode: Relocation,
  source: string,
  destination: string,
  outcome: ObjectOperationOutcome,
) {
  switch (outcome.kind) {
    case "completed":
      toast.success(t(`ops.${mode}Done`));
      break;
    case "conflict":
      conflict.value = { mode, source, destination, size: outcome.existingSize };
      conflictOpen.value = true;
      break;
    case "sourceKept":
      toast.warning(t("ops.sourceKept"), { description: outcome.reason, duration: 10_000 });
      break;
  }
}

async function confirmOverwrite() {
  const c = conflict.value;
  if (!c) return;
  try {
    await relocate(c.mode, c.source, c.destination, true);
  } catch (err) {
    toast.error(err instanceof CommandError ? err.appError.message : String(err));
  }
}

/** Rename/copy/move apply to one file at a time. */
const singleCheckedFile = computed(() => {
  const checked = explorer.checkedEntries;
  return checked.length === 1 && !checked[0]!.isPrefix ? checked[0] : undefined;
});

function openDelete(entries: StorageObject[]) {
  if (!entries.length) return;
  deleteTargets.value = entries;
  deleteOpen.value = true;
}

function onRowAction(action: "rename" | "copy" | "move" | "delete", entry: StorageObject) {
  if (action === "delete") {
    // Deleting a checked row acts on the whole selection.
    openDelete(explorer.checkedKeys.has(entry.key) ? explorer.checkedEntries : [entry]);
  } else {
    openPrompt(action, entry);
  }
}

const baseName = (key: string) => key.slice(key.lastIndexOf("/") + 1);

const { drag, onPointerDown, shouldSuppressClick } = useObjectDrag({
  // Dragging a checked row moves the whole checked set (files only).
  keysFor: (key) =>
    explorer.checkedKeys.has(key)
      ? explorer.checkedEntries.filter((e) => !e.isPrefix).map((e) => e.key)
      : [key],
  // Dropping into the current folder would be a no-op.
  canDrop: (keys, prefix) =>
    prefix !== props.prefix && keys.some((k) => prefix + baseName(k) !== k),
  onDrop: (keys, prefix) => void moveInto(keys, prefix),
});

/**
 * Moves files into `prefix`. A single file follows the normal conflict
 * dialog; for several files, existing destinations are skipped and reported
 * rather than overwritten.
 */
async function moveInto(keys: string[], prefix: string) {
  const moves = keys
    .map((source) => ({ source, destination: prefix + baseName(source) }))
    .filter((m) => m.source !== m.destination);
  if (moves.length === 1) {
    const [{ source, destination }] = moves as [(typeof moves)[0]];
    try {
      await relocate("move", source, destination, false);
    } catch (err) {
      toast.error(toAppError(err).message);
    }
    return;
  }

  let moved = 0;
  const skipped: string[] = [];
  const problems: string[] = [];
  for (const { source, destination } of moves) {
    try {
      const outcome = await explorer.relocate("move", source, destination, "ASK", {
        refresh: false,
      });
      if (outcome.kind === "completed") moved++;
      else if (outcome.kind === "conflict") skipped.push(baseName(source));
      else problems.push(`${baseName(source)}: ${outcome.reason}`);
    } catch (err) {
      problems.push(`${baseName(source)}: ${toAppError(err).message}`);
    }
  }
  explorer.setAllChecked([], false);
  await explorer.refresh();

  if (moved) toast.success(t("ops.movedMany", { count: moved }, moved));
  if (skipped.length) {
    toast.warning(t("ops.skippedExisting", { count: skipped.length }, skipped.length), {
      description: skipped.join(", "),
    });
  }
  if (problems.length) {
    toast.error(t("ops.moveProblems", { count: problems.length }, problems.length), {
      description: problems.join("\n"),
      duration: 10_000,
    });
  }
}

function goTo(prefix: string) {
  router.push({
    name: "explorer",
    params: { id: props.id, bucket: props.bucket },
    query: prefix ? { prefix } : {},
  });
}
</script>

<template>
  <div
    class="flex h-full min-h-0"
    @pointerdown="onPointerDown"
    @click.capture="(e) => shouldSuppressClick() && e.stopPropagation()"
  >
    <div class="flex min-w-0 flex-1 flex-col">
      <header class="flex h-11 shrink-0 items-center gap-2 border-b px-4">
        <Button
          variant="ghost"
          size="icon"
          :disabled="!prefix"
          :title="$t('explorer.up')"
          :aria-label="$t('explorer.up')"
          @click="goTo(parentPrefix(prefix))"
        >
          <ArrowUp />
        </Button>
        <ExplorerBreadcrumb
          class="flex-1"
          :connection-id="id"
          :connection-name="connectionName"
          :bucket="bucket"
          :prefix="prefix"
        />
        <template v-if="explorer.checkedKeys.size">
          <span class="text-muted-foreground text-xs whitespace-nowrap">
            {{ $t("explorer.selected", { count: explorer.checkedKeys.size }) }}
          </span>
          <template v-if="singleCheckedFile">
            <Button variant="outline" size="sm" @click="openPrompt('rename', singleCheckedFile)">
              <Pencil /> {{ $t("ops.rename") }}
            </Button>
            <Button variant="outline" size="sm" @click="openPrompt('copy', singleCheckedFile)">
              <Copy /> {{ $t("ops.copy") }}
            </Button>
            <Button variant="outline" size="sm" @click="openPrompt('move', singleCheckedFile)">
              <FolderInput /> {{ $t("ops.move") }}
            </Button>
          </template>
          <Button variant="outline" size="sm" @click="openDelete(explorer.checkedEntries)">
            <Trash2 /> {{ $t("common.delete") }}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            :aria-label="$t('explorer.clearSelection')"
            @click="explorer.setAllChecked([], false)"
          >
            <X />
          </Button>
        </template>
        <Button
          v-else
          variant="ghost"
          size="sm"
          :disabled="explorer.loading || !!explorer.error"
          @click="openPrompt('newFolder')"
        >
          <FolderPlus /> {{ $t("ops.newFolder") }}
        </Button>
        <label class="relative">
          <Search
            class="text-muted-foreground absolute top-1/2 left-2 size-3.5 -translate-y-1/2"
            aria-hidden="true"
          />
          <input
            v-model="filter"
            type="search"
            :placeholder="$t('explorer.filter')"
            :aria-label="$t('explorer.filter')"
            class="bg-background focus-visible:ring-ring/50 h-8 w-48 rounded-md border pr-2 pl-7 text-sm outline-none focus-visible:ring-2"
          />
        </label>
        <Button
          variant="ghost"
          size="icon"
          :disabled="explorer.loading"
          :title="$t('explorer.refresh')"
          :aria-label="$t('explorer.refresh')"
          @click="explorer.refresh()"
        >
          <RefreshCw :class="{ 'animate-spin': explorer.loading }" />
        </Button>
      </header>

      <div class="min-h-0 flex-1 overflow-auto">
        <div
          v-if="explorer.error"
          class="border-destructive/40 bg-destructive/10 text-destructive m-4 grid gap-1 rounded-md border p-3 text-sm"
          role="alert"
        >
          <div class="flex gap-2">
            <CircleAlert class="mt-0.5 size-4 shrink-0" />
            <span>{{ explorer.error.message }}</span>
          </div>
          <code v-if="explorer.error.details" class="pl-6 font-mono text-xs opacity-80">
            {{ explorer.error.details }}
          </code>
        </div>

        <p v-if="explorer.loading" class="text-muted-foreground p-4 text-sm">
          {{ $t("explorer.loading") }}
        </p>
        <p
          v-else-if="!explorer.error && !explorer.entries.length"
          class="text-muted-foreground p-8 text-center text-sm"
        >
          {{ $t("explorer.empty") }}
        </p>
        <template v-else-if="explorer.entries.length">
          <ObjectTable
            :entries="visible"
            :selected-key="explorer.selectedKey"
            :checked-keys="explorer.checkedKeys"
            @open-folder="goTo"
            @select="explorer.select"
            @toggle="explorer.toggleChecked"
            @toggle-all="
              (checked) =>
                explorer.setAllChecked(
                  visible.map((e) => e.key),
                  checked,
                )
            "
            @action="onRowAction"
          />
          <p v-if="filter && !visible.length" class="text-muted-foreground p-4 text-center text-sm">
            {{ $t("explorer.noMatches") }}
          </p>
          <div v-if="explorer.isTruncated" class="flex flex-col items-center gap-1 p-4">
            <Button
              variant="outline"
              size="sm"
              :disabled="explorer.loadingMore"
              @click="explorer.loadMore()"
            >
              {{ explorer.loadingMore ? $t("explorer.loading") : $t("explorer.loadMore") }}
            </Button>
            <p v-if="filter" class="text-muted-foreground text-xs">
              {{ $t("explorer.filterLoadedOnly") }}
            </p>
          </div>
        </template>
      </div>

      <footer
        v-if="explorer.entries.length"
        class="text-muted-foreground flex h-7 shrink-0 items-center border-t px-4 text-xs"
      >
        {{ $t("explorer.count", { count: explorer.entries.length }, explorer.entries.length) }}
        <template v-if="explorer.isTruncated">+</template>
      </footer>
    </div>

    <ObjectDetails
      v-if="explorer.selected"
      @action="(action) => explorer.selected && onRowAction(action, explorer.selected)"
    />

    <PromptDialog
      v-model:open="promptOpen"
      :title="promptConfig.title"
      :description="prompt.entry?.key"
      :label="promptConfig.label"
      :initial-value="promptConfig.initialValue"
      :confirm-label="promptConfig.confirmLabel"
      :select-range="promptConfig.selectRange"
      :submit="submitPrompt"
    />
    <ConfirmDialog
      v-model:open="conflictOpen"
      :title="$t('ops.conflictTitle')"
      :description="
        $t('ops.conflictDescription', {
          key: conflict?.destination ?? '',
          size: formatBytes(conflict?.size),
        })
      "
      :confirm-label="$t('ops.overwrite')"
      destructive
      @confirm="confirmOverwrite"
    />
    <DeleteObjectsDialog v-model:open="deleteOpen" :entries="deleteTargets" />

    <!-- Drag preview following the pointer. -->
    <div
      v-if="drag"
      class="bg-popover text-popover-foreground pointer-events-none fixed z-50 rounded-md border px-2.5 py-1 text-xs shadow-md"
      :style="{ left: `${drag.x + 12}px`, top: `${drag.y + 12}px` }"
    >
      <template v-if="drag.targetPrefix !== null">
        {{
          $t(
            "ops.dragMoveTo",
            { count: drag.keys.length, folder: drag.targetPrefix || bucket },
            drag.keys.length,
          )
        }}
      </template>
      <template v-else>
        {{ $t("ops.dragging", { count: drag.keys.length }, drag.keys.length) }}
      </template>
    </div>
  </div>
</template>
