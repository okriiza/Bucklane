<script setup lang="ts">
import { ArrowRight, CopyPlus, FolderInput, Info } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import ProviderMark from "@/components/connections/ProviderMark.vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import Input from "@/components/ui/input/Input.vue";
import Label from "@/components/ui/label/Label.vue";
import NativeSelect from "@/components/ui/native-select/NativeSelect.vue";
import { commands, type RelocateMode, type StorageObject } from "@/services/tauri";
import { useConnectionStore } from "@/stores/connections";

export interface CopyDestination {
  connectionId: string;
  bucket: string;
  /** Empty for the bucket root, otherwise ending in `/`. */
  prefix: string;
  mode: RelocateMode;
}

const props = defineProps<{
  connectionId: string;
  bucket: string;
  entries: StorageObject[];
  /** Copy or move when the dialog opens; the user can switch. */
  initialMode?: RelocateMode;
}>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ copy: [destination: CopyDestination] }>();
const { t } = useI18n();
const connections = useConnectionStore();

const mode = ref<RelocateMode>("COPY");
const destinationId = ref("");
const destinationBucket = ref("");
const folder = ref("");
const buckets = ref<string[]>([]);
const bucketsLoading = ref(false);

const source = computed(() => connections.byId(props.connectionId));
const destination = computed(() => connections.byId(destinationId.value));
const summary = computed(() =>
  props.entries.length === 1
    ? (props.entries[0]?.name ?? "")
    : t("copyTo.items", { count: props.entries.length }, props.entries.length),
);
const prefix = computed(() => {
  const trimmed = folder.value.trim().replace(/^\/+/, "");
  return trimmed && !trimmed.endsWith("/") ? `${trimmed}/` : trimmed;
});
const sameAsSource = computed(
  () =>
    destinationId.value === props.connectionId &&
    destinationBucket.value.trim() === props.bucket &&
    // Copying into another folder of the same bucket is fine.
    props.entries.every((e) => parentOf(e.key) === prefix.value),
);
const intoItself = computed(
  () =>
    mode.value === "MOVE" &&
    destinationId.value === props.connectionId &&
    destinationBucket.value.trim() === props.bucket &&
    props.entries.some((e) => e.isPrefix && prefix.value.startsWith(e.key)),
);
const blocked = computed(
  () => !destinationBucket.value.trim() || sameAsSource.value || intoItself.value,
);

function parentOf(key: string) {
  const trimmed = key.endsWith("/") ? key.slice(0, -1) : key;
  return trimmed.slice(0, trimmed.lastIndexOf("/") + 1);
}

watch(open, (isOpen) => {
  if (!isOpen) return;
  // Copying usually goes to another connection; default to the first one.
  const other = connections.connections.find((c) => c.id !== props.connectionId);
  destinationId.value = other?.id ?? props.connectionId;
  folder.value = "";
  mode.value = props.initialMode ?? "COPY";
});

// Suggestions only: a bucket the keys can't list can still be typed in.
watch(destinationId, async (id) => {
  destinationBucket.value = id === props.connectionId ? props.bucket : "";
  buckets.value = [];
  if (!id) return;
  bucketsLoading.value = true;
  const [listed, bookmarks] = await Promise.all([
    commands.listBuckets(id),
    commands.listBucketBookmarks(id),
  ]);
  if (destinationId.value !== id) return;
  const names = new Set<string>();
  if (listed.status === "ok") listed.data.forEach((b) => names.add(b.name));
  if (bookmarks.status === "ok") bookmarks.data.forEach((b) => names.add(b.bucketName));
  buckets.value = [...names].sort((a, b) => a.localeCompare(b));
  if (!destinationBucket.value && buckets.value.length === 1)
    destinationBucket.value = buckets.value[0] ?? "";
  bucketsLoading.value = false;
});

function submit() {
  if (blocked.value) return;
  emit("copy", {
    connectionId: destinationId.value,
    bucket: destinationBucket.value.trim(),
    prefix: prefix.value,
    mode: mode.value,
  });
  open.value = false;
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-lg">
      <div class="space-y-1">
        <DialogTitle class="flex items-center gap-2 text-lg font-semibold">
          <FolderInput v-if="mode === 'MOVE'" class="text-primary size-5" />
          <CopyPlus v-else class="text-primary size-5" />
          {{ mode === "MOVE" ? $t("copyTo.moveTitle") : $t("copyTo.title") }}
        </DialogTitle>
        <DialogDescription class="text-muted-foreground text-[13px] break-all">
          {{ summary }}
        </DialogDescription>
      </div>

      <div
        role="radiogroup"
        :aria-label="$t('copyTo.mode')"
        class="bg-muted grid grid-cols-2 gap-1 rounded-lg p-1 text-[13px]"
      >
        <button
          v-for="option in ['COPY', 'MOVE'] as const"
          :key="option"
          type="button"
          role="radio"
          :aria-checked="mode === option"
          class="flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition-colors [&_svg]:size-4"
          :class="
            mode === option
              ? 'bg-background text-foreground shadow-[var(--shadow-soft)]'
              : 'text-muted-foreground hover:text-foreground'
          "
          @click="mode = option"
        >
          <CopyPlus v-if="option === 'COPY'" />
          <FolderInput v-else />
          {{ option === "COPY" ? $t("copyTo.copyMode") : $t("copyTo.moveMode") }}
        </button>
      </div>

      <div class="bg-muted/50 flex items-center gap-2 rounded-lg border p-2.5 text-xs">
        <ProviderMark v-if="source" :provider="source.provider" />
        <div class="min-w-0 flex-1">
          <div class="truncate font-medium">{{ source?.name }}</div>
          <div class="text-muted-foreground truncate font-mono">{{ props.bucket }}/</div>
        </div>
        <ArrowRight class="text-muted-foreground size-4 shrink-0" />
        <ProviderMark v-if="destination" :provider="destination.provider" />
        <div class="min-w-0 flex-1">
          <div class="truncate font-medium">{{ destination?.name ?? "—" }}</div>
          <div class="text-muted-foreground truncate font-mono">
            {{ destinationBucket.trim() || "…" }}/{{ prefix }}
          </div>
        </div>
      </div>

      <form class="grid gap-4" @submit.prevent="submit">
        <div class="grid gap-1.5">
          <Label for="copy-connection">{{ $t("copyTo.connection") }}</Label>
          <NativeSelect id="copy-connection" v-model="destinationId">
            <option v-for="c in connections.connections" :key="c.id" :value="c.id">
              {{ c.name }}{{ c.id === connectionId ? ` (${$t("copyTo.current")})` : "" }}
            </option>
          </NativeSelect>
        </div>
        <div class="grid gap-1.5">
          <Label for="copy-bucket">{{ $t("copyTo.bucket") }}</Label>
          <Input
            id="copy-bucket"
            v-model="destinationBucket"
            list="copy-buckets"
            autocomplete="off"
            spellcheck="false"
            :placeholder="bucketsLoading ? $t('explorer.loading') : $t('copyTo.bucketPlaceholder')"
            class="font-mono text-xs"
          />
          <datalist id="copy-buckets">
            <option v-for="name in buckets" :key="name" :value="name" />
          </datalist>
        </div>
        <div class="grid gap-1.5">
          <Label for="copy-folder">
            {{ $t("copyTo.folder") }}
            <span class="text-muted-foreground font-normal">({{ $t("common.optional") }})</span>
          </Label>
          <Input
            id="copy-folder"
            v-model="folder"
            autocomplete="off"
            spellcheck="false"
            placeholder="backups/2026/"
            class="font-mono text-xs"
          />
          <p class="text-muted-foreground text-xs">{{ $t("copyTo.folderHint") }}</p>
        </div>

        <p v-if="sameAsSource" class="text-destructive text-xs" role="alert">
          {{ $t("copyTo.sameAsSource") }}
        </p>
        <p v-else-if="intoItself" class="text-destructive text-xs" role="alert">
          {{ $t("copyTo.intoItself") }}
        </p>
        <p class="text-muted-foreground flex gap-2 text-xs">
          <Info class="mt-px size-4 shrink-0" />
          {{ mode === "MOVE" ? $t("copyTo.moveNote") : $t("copyTo.note") }}
        </p>

        <div class="flex justify-end gap-2">
          <Button variant="ghost" type="button" @click="open = false">
            {{ $t("common.cancel") }}
          </Button>
          <Button type="submit" :disabled="blocked">
            <FolderInput v-if="mode === 'MOVE'" />
            <CopyPlus v-else />
            {{ mode === "MOVE" ? $t("copyTo.moveSubmit") : $t("copyTo.submit") }}
          </Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
