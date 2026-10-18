<script setup lang="ts">
import {
  ArrowUp,
  CircleAlert,
  Copy,
  Download,
  FolderInput,
  FolderOpen,
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
import ObjectTable, { type RowAction } from "@/components/explorer/ObjectTable.vue";
import PreviewDialog from "@/components/explorer/PreviewDialog.vue";
import ShareLinkDialog from "@/components/explorer/ShareLinkDialog.vue";
import UploadButton from "@/components/explorer/UploadButton.vue";
import { Button } from "@/components/ui/button";
import { CommandError, type ObjectOperationOutcome, type StorageObject } from "@/services/tauri";
import { useObjectDrag } from "@/composables/useObjectDrag";
import { useTransferActions } from "@/composables/useTransferActions";
import { toAppError } from "@/services/tauri";
import { useConnectionStore } from "@/stores/connections";
import { useExplorerStore } from "@/stores/explorer";
import { copyText, s3Uri } from "@/utils/clipboard";
import { formatBytes } from "@/utils/format";
import { extensionOf, parentPrefix } from "@/utils/prefix";
import { nextSort } from "@/utils/sort";

const props = defineProps<{ id: string; bucket: string; prefix: string }>();

const router = useRouter();
const { pickAndUpload, pickAndUploadFolder, pickAndDownload } = useTransferActions();
const uploadTarget = computed(() => ({
  connectionId: props.id,
  bucket: props.bucket,
  prefix: props.prefix,
}));
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

// Client-side filter over loaded entries only (EXP-004). Enter runs a
// server-side name-prefix search over the whole folder instead.
const visible = computed(() => {
  const q = filter.value.trim().toLowerCase();
  if (!q) return explorer.sortedEntries;
  return explorer.sortedEntries.filter((e) => e.name.toLowerCase().includes(q));
});

const query = computed(() => filter.value.trim());
/** Offer a server search when the loaded items may not be the whole folder. */
const canSearchServer = computed(
  () =>
    !!query.value &&
    !query.value.includes("/") &&
    query.value !== explorer.search &&
    (explorer.isTruncated || !!explorer.search),
);

function runSearch() {
  if (!query.value) return void clearSearch();
  if (query.value.includes("/")) return void toast.info(t("explorer.searchNoSlash"));
  void explorer.searchServer(query.value);
}

function clearSearch() {
  filter.value = "";
  void explorer.clearSearch();
}

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

/** Files download directly; folders download recursively. */
function downloadEntries(entries: StorageObject[]) {
  return pickAndDownload(
    props.id,
    props.bucket,
    entries.map((e) => e.key),
  );
}

const shareOpen = ref(false);
const shareKey = ref("");
const insecureConnection = computed(() => connections.byId(props.id)?.allowInsecureHttp ?? false);

async function copyToClipboard(text: string, label: string) {
  try {
    await copyText(text);
    toast.success(t("share.copiedValue", { label }));
  } catch {
    toast.error(t("share.copyFailed"));
  }
}

const previewOpen = ref(false);
const previewKey = ref("");

function onRowAction(action: RowAction, entry: StorageObject) {
  if (action === "preview") {
    previewKey.value = entry.key;
    previewOpen.value = true;
  } else if (action === "share") {
    shareKey.value = entry.key;
    shareOpen.value = true;
  } else if (action === "copyKey") {
    void copyToClipboard(entry.key, t("share.copyKeyLabel"));
  } else if (action === "copyUri") {
    void copyToClipboard(s3Uri(props.bucket, entry.key), t("share.copyUriLabel"));
  } else if (action === "download") {
    // Downloading a checked row acts on the whole selection.
    void downloadEntries(explorer.checkedKeys.has(entry.key) ? explorer.checkedEntries : [entry]);
  } else if (action === "delete") {
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
      <header data-tauri-drag-region class="flex h-12 shrink-0 items-center gap-1.5 px-3">
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
          class="min-w-0 flex-1"
          :connection-id="id"
          :connection-name="connectionName"
          :bucket="bucket"
          :prefix="prefix"
        />
        <label class="relative">
          <Search
            class="text-muted-foreground absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2"
            aria-hidden="true"
          />
          <input
            v-model="filter"
            type="search"
            :placeholder="$t('explorer.filter')"
            :aria-label="$t('explorer.filter')"
            :title="$t('explorer.searchHint')"
            class="bg-muted/60 focus-visible:bg-background focus-visible:ring-ring/25 focus-visible:border-ring placeholder:text-muted-foreground/70 h-8 w-52 rounded-md border border-transparent pr-2 pl-8 text-[13px] transition-[background-color,box-shadow] outline-none focus-visible:ring-[3px]"
            @keydown.enter.prevent="runSearch"
            @keydown.esc="filter ? (filter = '') : clearSearch()"
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

      <div
        class="flex h-11 shrink-0 items-center gap-1.5 border-y px-3 transition-colors"
        :class="explorer.checkedKeys.size ? 'bg-primary/8 border-primary/15' : ''"
      >
        <template v-if="explorer.checkedKeys.size">
          <Button
            variant="ghost"
            size="icon-sm"
            :aria-label="$t('explorer.clearSelection')"
            :title="$t('explorer.clearSelection')"
            @click="explorer.setAllChecked([], false)"
          >
            <X />
          </Button>
          <span class="text-primary mr-2 text-[13px] font-medium whitespace-nowrap">
            {{ $t("explorer.selected", { count: explorer.checkedKeys.size }) }}
          </span>
          <Button variant="outline" size="sm" @click="downloadEntries(explorer.checkedEntries)">
            <Download /> {{ $t("transfers.downloadAction") }}
          </Button>
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
          <Button
            variant="outline"
            size="sm"
            class="hover:text-destructive ml-auto"
            @click="openDelete(explorer.checkedEntries)"
          >
            <Trash2 /> {{ $t("common.delete") }}
          </Button>
        </template>
        <template v-else>
          <UploadButton
            :disabled="explorer.loading || !!explorer.error"
            @files="pickAndUpload(uploadTarget)"
            @folder="pickAndUploadFolder(uploadTarget)"
          />
          <Button
            variant="ghost"
            size="sm"
            :disabled="explorer.loading || !!explorer.error"
            @click="openPrompt('newFolder')"
          >
            <FolderPlus /> {{ $t("ops.newFolder") }}
          </Button>
          <span class="text-muted-foreground ml-auto hidden text-[11px] lg:inline">
            {{ $t("explorer.dropHint") }}
          </span>
        </template>
      </div>

      <div
        v-if="explorer.search"
        class="bg-muted/40 flex h-9 shrink-0 items-center gap-2 border-b px-4 text-[12px]"
      >
        <Search class="text-muted-foreground size-3.5" aria-hidden="true" />
        <span class="truncate">{{ $t("explorer.searchActive", { query: explorer.search }) }}</span>
        <Button variant="ghost" size="sm" class="ml-auto h-6 px-2 text-xs" @click="clearSearch">
          <X /> {{ $t("explorer.clearSearch") }}
        </Button>
      </div>

      <div class="min-h-0 flex-1 overflow-auto">
        <div
          v-if="explorer.error"
          class="border-destructive/30 bg-destructive/8 text-destructive m-4 grid gap-1 rounded-lg border p-3 text-[13px]"
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

        <div v-if="explorer.loading" aria-busy="true" :aria-label="$t('explorer.loading')">
          <div
            v-for="n in 9"
            :key="n"
            class="flex h-9 items-center gap-3 border-b px-4"
            :style="{ opacity: 1 - n * 0.08 }"
          >
            <span class="skeleton size-3.5 rounded" />
            <span class="skeleton size-4 rounded" />
            <span class="skeleton h-3 rounded" :style="{ width: `${18 + ((n * 37) % 30)}%` }" />
            <span class="skeleton ml-auto h-3 w-14 rounded" />
            <span class="skeleton h-3 w-28 rounded" />
          </div>
        </div>
        <div
          v-else-if="!explorer.error && !explorer.entries.length && explorer.search"
          class="text-muted-foreground flex flex-col items-center gap-3 p-10 text-center text-[13px]"
        >
          {{ $t("explorer.searchNoResults", { query: explorer.search }) }}
          <Button variant="outline" size="sm" @click="clearSearch">
            {{ $t("explorer.clearSearch") }}
          </Button>
        </div>
        <div
          v-else-if="!explorer.error && !explorer.entries.length"
          class="flex h-full min-h-64 flex-col items-center justify-center gap-3 p-8 text-center"
        >
          <span
            class="bg-primary/10 text-primary inline-flex size-12 items-center justify-center rounded-xl"
          >
            <FolderOpen class="size-6" />
          </span>
          <div>
            <p class="text-[15px] font-medium">{{ $t("explorer.empty") }}</p>
            <p class="text-muted-foreground mt-1 text-[13px]">{{ $t("explorer.emptyHint") }}</p>
          </div>
          <UploadButton
            @files="pickAndUpload(uploadTarget)"
            @folder="pickAndUploadFolder(uploadTarget)"
          />
        </div>
        <template v-else-if="explorer.entries.length">
          <ObjectTable
            :entries="visible"
            :selected-key="explorer.selectedKey"
            :checked-keys="explorer.checkedKeys"
            :sort="explorer.sort"
            @sort="(key) => explorer.setSort(nextSort(explorer.sort, key))"
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
          <div v-if="canSearchServer" class="flex justify-center px-4 pt-3">
            <Button variant="secondary" size="sm" @click="runSearch">
              <Search /> {{ $t("explorer.searchServer", { query }) }}
            </Button>
          </div>
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
        class="text-muted-foreground tabular flex h-7 shrink-0 items-center border-t px-4 text-[11px]"
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
    <PreviewDialog
      v-model:open="previewOpen"
      :connection-id="id"
      :bucket="bucket"
      :object-key="previewKey"
      @download="pickAndDownload(id, bucket, [previewKey])"
    />
    <ShareLinkDialog
      v-model:open="shareOpen"
      :connection-id="id"
      :bucket="bucket"
      :object-key="shareKey"
      :insecure="insecureConnection"
    />

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
