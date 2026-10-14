<script setup lang="ts">
import { ArrowUp, CircleAlert, RefreshCw, Search } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";

import ExplorerBreadcrumb from "@/components/explorer/ExplorerBreadcrumb.vue";
import ObjectDetails from "@/components/explorer/ObjectDetails.vue";
import ObjectTable from "@/components/explorer/ObjectTable.vue";
import { Button } from "@/components/ui/button";
import { useConnectionStore } from "@/stores/connections";
import { useExplorerStore } from "@/stores/explorer";
import { parentPrefix } from "@/utils/prefix";

const props = defineProps<{ id: string; bucket: string; prefix: string }>();

const router = useRouter();
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

function goTo(prefix: string) {
  router.push({
    name: "explorer",
    params: { id: props.id, bucket: props.bucket },
    query: prefix ? { prefix } : {},
  });
}
</script>

<template>
  <div class="flex h-full min-h-0">
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
            @open-folder="goTo"
            @select="explorer.select"
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

    <ObjectDetails v-if="explorer.selected" />
  </div>
</template>
