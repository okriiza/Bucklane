<script setup lang="ts">
import { File, FileArchive, FileImage, FileText, FileVideo, Folder } from "@lucide/vue";
import { computed } from "vue";

import type { StorageObject } from "@/services/tauri";
import { formatBytes, formatDateTime } from "@/utils/format";
import { extensionOf } from "@/utils/prefix";

import RowActions from "./RowActions.vue";

const props = defineProps<{
  entries: StorageObject[];
  selectedKey: string | null;
  checkedKeys: Set<string>;
}>();
const emit = defineEmits<{
  openFolder: [prefix: string];
  select: [key: string];
  toggle: [key: string, checked: boolean];
  toggleAll: [checked: boolean];
  action: [action: "download" | "rename" | "copy" | "move" | "delete", entry: StorageObject];
}>();

const ICONS: Record<string, typeof File> = {
  jpg: FileImage,
  jpeg: FileImage,
  png: FileImage,
  gif: FileImage,
  webp: FileImage,
  svg: FileImage,
  mp4: FileVideo,
  mov: FileVideo,
  webm: FileVideo,
  zip: FileArchive,
  gz: FileArchive,
  tar: FileArchive,
  txt: FileText,
  md: FileText,
  json: FileText,
  csv: FileText,
  pdf: FileText,
};

const allChecked = computed(
  () => props.entries.length > 0 && props.entries.every((e) => props.checkedKeys.has(e.key)),
);
const someChecked = computed(
  () => !allChecked.value && props.entries.some((e) => props.checkedKeys.has(e.key)),
);

function iconFor(entry: StorageObject) {
  if (entry.isPrefix) return Folder;
  return ICONS[extensionOf(entry.name)] ?? File;
}

function activate(entry: StorageObject) {
  if (entry.isPrefix) emit("openFolder", entry.key);
  else emit("select", entry.key);
}
</script>

<template>
  <table class="w-full table-fixed text-[13px]">
    <thead class="bg-background/95 sticky top-0 z-10 backdrop-blur">
      <tr class="text-muted-foreground border-b text-left text-[11px]">
        <th class="w-10 py-2 pl-4">
          <input
            type="checkbox"
            class="accent-primary size-3.5 align-middle"
            :checked="allChecked"
            :indeterminate="someChecked"
            :aria-label="$t('explorer.selectAll')"
            @change="emit('toggleAll', ($event.target as HTMLInputElement).checked)"
          />
        </th>
        <th class="px-2 py-2 font-medium">{{ $t("explorer.name") }}</th>
        <th class="w-24 px-4 py-2 text-right font-medium">{{ $t("explorer.size") }}</th>
        <th class="w-44 px-4 py-2 font-medium">{{ $t("explorer.lastModified") }}</th>
        <th class="w-12" />
      </tr>
    </thead>
    <tbody>
      <tr
        v-for="entry in entries"
        :key="entry.key"
        tabindex="0"
        class="group hover:bg-accent/70 focus-visible:bg-accent h-9 cursor-default border-b border-border/60 transition-colors outline-none last:border-b-0"
        :class="{
          'bg-primary/8 hover:bg-primary/12':
            entry.key === selectedKey || checkedKeys.has(entry.key),
        }"
        :aria-selected="entry.key === selectedKey"
        :data-drag-key="entry.isPrefix ? undefined : entry.key"
        :data-drop-prefix="entry.isPrefix ? entry.key : undefined"
        @click="activate(entry)"
        @keydown.enter.self.prevent="activate(entry)"
        @keydown.delete.self.prevent="emit('action', 'delete', entry)"
        @keydown.f2.self.prevent="!entry.isPrefix && emit('action', 'rename', entry)"
      >
        <td class="pl-4" @click.stop>
          <input
            type="checkbox"
            class="accent-primary size-3.5 align-middle"
            :checked="checkedKeys.has(entry.key)"
            :aria-label="$t('explorer.select', { name: entry.name })"
            @change="emit('toggle', entry.key, ($event.target as HTMLInputElement).checked)"
          />
        </td>
        <td class="px-2">
          <span class="flex min-w-0 items-center gap-2.5">
            <component
              :is="iconFor(entry)"
              class="size-4 shrink-0"
              :class="entry.isPrefix ? 'text-primary fill-primary/15' : 'text-muted-foreground'"
            />
            <span class="truncate" :class="{ 'font-medium': entry.isPrefix }" :title="entry.key">
              {{ entry.name }}
            </span>
          </span>
        </td>
        <td class="text-muted-foreground tabular px-4 text-right font-mono text-xs">
          {{ entry.isPrefix ? "" : formatBytes(entry.size) }}
        </td>
        <td class="text-muted-foreground tabular truncate px-4 text-xs">
          {{ entry.isPrefix ? "" : formatDateTime(entry.lastModified) }}
        </td>
        <td class="pr-2 text-right" @click.stop>
          <RowActions
            :entry="entry"
            @details="emit('select', entry.key)"
            @download="emit('action', 'download', entry)"
            @rename="emit('action', 'rename', entry)"
            @copy="emit('action', 'copy', entry)"
            @move="emit('action', 'move', entry)"
            @delete="emit('action', 'delete', entry)"
          />
        </td>
      </tr>
    </tbody>
  </table>
</template>
