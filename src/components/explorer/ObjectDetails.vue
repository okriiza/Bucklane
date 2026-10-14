<script setup lang="ts">
import { X } from "@lucide/vue";

import { Button } from "@/components/ui/button";
import { useExplorerStore } from "@/stores/explorer";
import { formatBytes, formatDateTime } from "@/utils/format";

const explorer = useExplorerStore();
</script>

<!-- Object metadata panel (EXP-005). Metadata comes from HeadObject, not the listing. -->
<template>
  <aside class="flex w-80 shrink-0 flex-col border-l" :aria-label="$t('explorer.details')">
    <header class="flex h-11 items-center justify-between border-b pr-2 pl-4">
      <h2 class="text-sm font-semibold">{{ $t("explorer.details") }}</h2>
      <Button
        variant="ghost"
        size="icon"
        :aria-label="$t('explorer.closeDetails')"
        @click="explorer.clearSelection()"
      >
        <X />
      </Button>
    </header>

    <div class="min-h-0 flex-1 overflow-auto p-4 text-sm">
      <p class="mb-4 font-medium break-all select-text">{{ explorer.selected?.name }}</p>

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
