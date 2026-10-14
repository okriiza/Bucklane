<script setup lang="ts">
import { ChevronRight } from "@lucide/vue";
import { computed } from "vue";

import { prefixSegments } from "@/utils/prefix";

const props = defineProps<{
  connectionId: string;
  connectionName: string;
  bucket: string;
  prefix: string;
}>();

const segments = computed(() => prefixSegments(props.prefix));

function folderRoute(prefix: string) {
  return {
    name: "explorer",
    params: { id: props.connectionId, bucket: props.bucket },
    query: prefix ? { prefix } : {},
  };
}
</script>

<template>
  <nav :aria-label="$t('explorer.breadcrumb')" class="min-w-0">
    <ol class="flex min-w-0 items-center gap-1 text-sm">
      <li class="shrink-0">
        <RouterLink
          :to="{ name: 'connection', params: { id: connectionId } }"
          class="text-muted-foreground hover:text-foreground"
        >
          {{ connectionName }}
        </RouterLink>
      </li>
      <ChevronRight class="text-muted-foreground size-3.5 shrink-0" aria-hidden="true" />
      <li class="min-w-0 shrink-0">
        <RouterLink
          v-if="segments.length"
          :to="folderRoute('')"
          class="text-muted-foreground hover:text-foreground"
        >
          {{ bucket }}
        </RouterLink>
        <span v-else class="font-medium" aria-current="page">{{ bucket }}</span>
      </li>
      <template v-for="(segment, index) in segments" :key="segment.prefix">
        <ChevronRight class="text-muted-foreground size-3.5 shrink-0" aria-hidden="true" />
        <li class="min-w-0">
          <span
            v-if="index === segments.length - 1"
            class="block truncate font-medium"
            aria-current="page"
          >
            {{ segment.name || "∅" }}
          </span>
          <RouterLink
            v-else
            :to="folderRoute(segment.prefix)"
            class="text-muted-foreground hover:text-foreground block truncate"
          >
            {{ segment.name || "∅" }}
          </RouterLink>
        </li>
      </template>
    </ol>
  </nav>
</template>
