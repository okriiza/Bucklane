<script setup lang="ts">
import {
  Copy,
  Download,
  Eye,
  FolderInput,
  Link2,
  Pencil,
  SlidersHorizontal,
  Trash2,
  X,
} from "@lucide/vue";

import { computed } from "vue";

import { Button } from "@/components/ui/button";
import { useExplorerStore } from "@/stores/explorer";
import { formatBytes, formatDateTime } from "@/utils/format";

const explorer = useExplorerStore();

/** Optional HTTP headers, shown only when set. */
const headers = computed(() => {
  const m = explorer.metadata;
  if (!m) return [];
  return [
    { label: "Cache-Control", value: m.cacheControl },
    { label: "Content-Disposition", value: m.contentDisposition },
    { label: "Content-Encoding", value: m.contentEncoding },
    { label: "Content-Language", value: m.contentLanguage },
  ].filter((h) => h.value);
});
const emit = defineEmits<{
  action: [
    action: "preview" | "download" | "share" | "metadata" | "rename" | "copy" | "move" | "delete",
  ];
}>();
</script>

<!-- Object metadata panel (EXP-005). Metadata comes from HeadObject, not the listing. -->
<template>
  <aside
    class="bg-sidebar/60 flex w-80 shrink-0 flex-col border-l"
    :aria-label="$t('explorer.details')"
  >
    <header class="flex h-12 items-center justify-between pr-2 pl-4">
      <h2 class="text-muted-foreground text-[13px] font-medium">{{ $t("explorer.details") }}</h2>
      <Button
        variant="ghost"
        size="icon"
        :aria-label="$t('explorer.closeDetails')"
        @click="explorer.clearSelection()"
      >
        <X />
      </Button>
    </header>

    <div class="min-h-0 flex-1 overflow-auto px-4 pb-4 text-[13px]">
      <p class="mb-3 text-[15px] leading-snug font-semibold tracking-tight break-all select-text">
        {{ explorer.selected?.name }}
      </p>
      <div class="mb-5 flex flex-wrap gap-1.5">
        <Button size="sm" @click="emit('action', 'preview')">
          <Eye /> {{ $t("preview.action") }}
        </Button>
        <Button variant="outline" size="sm" @click="emit('action', 'download')">
          <Download /> {{ $t("transfers.downloadAction") }}
        </Button>
        <Button variant="outline" size="sm" @click="emit('action', 'share')">
          <Link2 /> {{ $t("share.button") }}
        </Button>
        <Button variant="outline" size="sm" @click="emit('action', 'metadata')">
          <SlidersHorizontal /> {{ $t("metadata.action") }}
        </Button>
        <Button variant="outline" size="sm" @click="emit('action', 'rename')">
          <Pencil /> {{ $t("ops.rename") }}
        </Button>
        <Button variant="outline" size="sm" @click="emit('action', 'copy')">
          <Copy /> {{ $t("ops.copy") }}
        </Button>
        <Button variant="outline" size="sm" @click="emit('action', 'move')">
          <FolderInput /> {{ $t("ops.move") }}
        </Button>
        <Button
          variant="outline"
          size="sm"
          :aria-label="$t('common.delete')"
          :title="$t('common.delete')"
          @click="emit('action', 'delete')"
        >
          <Trash2 />
        </Button>
      </div>

      <p v-if="explorer.metadataLoading" class="text-muted-foreground">
        {{ $t("explorer.loading") }}
      </p>
      <p v-else-if="explorer.metadataError" class="text-destructive" role="alert">
        {{ explorer.metadataError.message }}
      </p>

      <dl v-else-if="explorer.metadata" class="grid gap-3">
        <div>
          <dt class="text-muted-foreground text-xs">{{ $t("explorer.key") }}</dt>
          <dd class="font-mono text-xs break-all select-text">{{ explorer.metadata.key }}</dd>
        </div>
        <div>
          <dt class="text-muted-foreground text-xs">{{ $t("explorer.size") }}</dt>
          <dd>
            {{ formatBytes(explorer.metadata.size) }}
            <span v-if="explorer.metadata.size != null" class="text-muted-foreground text-xs">
              ({{ explorer.metadata.size.toLocaleString() }} B)
            </span>
          </dd>
        </div>
        <div>
          <dt class="text-muted-foreground text-xs">{{ $t("explorer.lastModified") }}</dt>
          <dd>{{ formatDateTime(explorer.metadata.lastModified) }}</dd>
        </div>
        <div v-if="explorer.metadata.contentType">
          <dt class="text-muted-foreground text-xs">{{ $t("explorer.contentType") }}</dt>
          <dd class="font-mono text-xs select-text">{{ explorer.metadata.contentType }}</dd>
        </div>
        <div v-for="header in headers" :key="header.label">
          <dt class="text-muted-foreground text-xs">{{ header.label }}</dt>
          <dd class="font-mono text-xs break-all select-text">{{ header.value }}</dd>
        </div>
        <div v-if="explorer.metadata.etag">
          <dt class="text-muted-foreground text-xs">ETag</dt>
          <dd class="font-mono text-xs break-all select-text">{{ explorer.metadata.etag }}</dd>
        </div>
        <div v-if="explorer.metadata.storageClass">
          <dt class="text-muted-foreground text-xs">{{ $t("explorer.storageClass") }}</dt>
          <dd class="font-mono text-xs">{{ explorer.metadata.storageClass }}</dd>
        </div>
        <div v-if="Object.keys(explorer.metadata.metadata).length">
          <dt class="text-muted-foreground text-xs">{{ $t("explorer.userMetadata") }}</dt>
          <dd class="mt-1 grid gap-1">
            <div
              v-for="(value, name) in explorer.metadata.metadata"
              :key="name"
              class="font-mono text-xs break-all select-text"
            >
              <span class="text-muted-foreground">{{ name }}:</span> {{ value }}
            </div>
          </dd>
        </div>
      </dl>
    </div>
  </aside>
</template>
