<script setup lang="ts">
import { File, FileArchive, FileImage, FileText, FileVideo, Folder } from "@lucide/vue";

import type { StorageObject } from "@/services/tauri";
import { formatBytes, formatDateTime } from "@/utils/format";
import { extensionOf } from "@/utils/prefix";

defineProps<{
  entries: StorageObject[];
  selectedKey: string | null;
}>();
const emit = defineEmits<{
  openFolder: [prefix: string];
  select: [key: string];
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
  <table class="w-full table-fixed text-sm">
    <thead class="bg-background sticky top-0 z-10">
      <tr class="text-muted-foreground border-b text-left text-xs">
        <th class="px-4 py-2 font-medium">{{ $t("explorer.name") }}</th>
        <th class="w-28 px-4 py-2 text-right font-medium">{{ $t("explorer.size") }}</th>
        <th class="w-48 px-4 py-2 font-medium">{{ $t("explorer.lastModified") }}</th>
      </tr>
    </thead>
    <tbody>
      <tr
        v-for="entry in entries"
        :key="entry.key"
        tabindex="0"
        class="hover:bg-accent focus-visible:bg-accent cursor-default border-b outline-none last:border-b-0"
        :class="{ 'bg-accent': entry.key === selectedKey }"
        :aria-selected="entry.key === selectedKey"
        @click="activate(entry)"
        @keydown.enter.prevent="activate(entry)"
      >
        <td class="px-4 py-1.5">
          <span class="flex min-w-0 items-center gap-2">
            <component
              :is="iconFor(entry)"
              class="size-4 shrink-0"
              :class="entry.isPrefix ? 'text-sky-500' : 'text-muted-foreground'"
            />
            <span class="truncate" :title="entry.key">{{ entry.name }}</span>
          </span>
        </td>
        <td class="text-muted-foreground px-4 py-1.5 text-right tabular-nums">
          {{ entry.isPrefix ? "" : formatBytes(entry.size) }}
        </td>
        <td class="text-muted-foreground truncate px-4 py-1.5">
          {{ entry.isPrefix ? "" : formatDateTime(entry.lastModified) }}
        </td>
      </tr>
    </tbody>
  </table>
</template>
