<script setup lang="ts">
import { ChevronRight, Database, Plus, RefreshCw, Star } from "@lucide/vue";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import Input from "@/components/ui/input/Input.vue";
import { toAppError } from "@/services/tauri";
import { useBucketStore, type BucketEntry } from "@/stores/buckets";

const props = defineProps<{ connectionId: string }>();

const store = useBucketStore();
const { t } = useI18n();
const newBucket = ref("");
const adding = ref(false);

watch(
  () => props.connectionId,
  (id) => store.load(id),
  { immediate: true },
);

async function addBookmark() {
  const name = newBucket.value.trim();
  if (!name) return;
  adding.value = true;
  try {
    await store.addBookmark(name);
    newBucket.value = "";
  } catch (err) {
    toast.error(toAppError(err).message);
  } finally {
    adding.value = false;
  }
}

async function toggleBookmark(entry: BucketEntry) {
  try {
    if (entry.bookmark) await store.removeBookmark(entry.bookmark.id);
    else await store.addBookmark(entry.name);
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}

function bookmarkLabel(entry: BucketEntry) {
  return entry.bookmark ? t("buckets.removeBookmark") : t("buckets.addBookmark");
}
</script>

<template>
  <section class="mt-8">
    <header class="mb-2 flex items-center gap-2">
      <h2 class="text-[15px] font-semibold tracking-tight">{{ $t("buckets.title") }}</h2>
      <span
        v-if="store.entries.length"
        class="bg-secondary text-muted-foreground tabular rounded px-1.5 text-[11px] font-medium"
      >
        {{ store.entries.length }}
      </span>
      <Button
        variant="ghost"
        size="icon-sm"
        class="ml-auto"
        :disabled="store.loading"
        :title="$t('explorer.refresh')"
        :aria-label="$t('explorer.refresh')"
        @click="store.load(connectionId)"
      >
        <RefreshCw :class="{ 'animate-spin': store.loading }" />
      </Button>
    </header>

    <p
      v-if="store.listError"
      class="bg-muted text-muted-foreground mb-3 rounded-lg px-3.5 py-3 text-[13px]"
      role="note"
    >
      <template v-if="store.listError.code === 'ACCESS_DENIED'">
        {{ $t("buckets.listDenied") }}
      </template>
      <template v-else>{{ store.listError.message }}</template>
    </p>

    <ul v-if="store.loading && !store.entries.length" class="divide-y rounded-lg border">
      <li v-for="n in 4" :key="n" class="flex items-center gap-3 px-3 py-2.5">
        <span class="skeleton size-7 rounded-md" />
        <span class="skeleton h-3 rounded" :style="{ width: `${28 + n * 9}%` }" />
      </li>
    </ul>

    <p
      v-else-if="!store.entries.length && !store.listError"
      class="text-muted-foreground rounded-lg border border-dashed px-4 py-6 text-center text-[13px]"
    >
      {{ $t("buckets.empty") }}
    </p>

    <ul
      v-if="store.entries.length"
      class="bg-card divide-y overflow-hidden rounded-lg border shadow-[var(--shadow-soft)]"
    >
      <li
        v-for="entry in store.entries"
        :key="entry.name"
        class="group hover:bg-accent/60 flex items-center transition-colors"
      >
        <RouterLink
          :to="{ name: 'explorer', params: { id: connectionId, bucket: entry.name } }"
          class="flex min-w-0 flex-1 items-center gap-3 px-3 py-2"
        >
          <span
            class="bg-primary/10 text-primary inline-flex size-7 shrink-0 items-center justify-center rounded-md"
          >
            <Database class="size-3.5" />
          </span>
          <span class="min-w-0">
            <span class="block truncate text-[13px] font-medium">{{ entry.label }}</span>
            <span
              v-if="entry.label !== entry.name || !entry.listed"
              class="text-muted-foreground block truncate text-[11px]"
            >
              <span class="font-mono">{{ entry.name }}</span>
              <template v-if="!entry.listed"> · {{ $t("buckets.bookmarkedOnly") }}</template>
            </span>
          </span>
          <span
            v-if="entry.region"
            class="bg-secondary text-muted-foreground ml-auto rounded px-1.5 py-0.5 font-mono text-[11px]"
          >
            {{ entry.region }}
          </span>
          <ChevronRight
            class="text-muted-foreground size-4 shrink-0 opacity-0 transition-opacity group-hover:opacity-100"
            :class="{ 'ml-auto': !entry.region }"
          />
        </RouterLink>
        <button
          type="button"
          class="hover:bg-accent mr-2 inline-flex size-7 items-center justify-center rounded-md transition-colors active:scale-90"
          :title="bookmarkLabel(entry)"
          :aria-label="bookmarkLabel(entry)"
          :aria-pressed="!!entry.bookmark"
          @click="toggleBookmark(entry)"
        >
          <Star
            class="size-3.5 transition-colors"
            :class="
              entry.bookmark
                ? 'fill-warning text-warning'
                : 'text-muted-foreground/60 group-hover:text-muted-foreground'
            "
          />
        </button>
      </li>
    </ul>

    <form class="mt-3 flex max-w-md gap-2" @submit.prevent="addBookmark">
      <Input
        v-model="newBucket"
        :placeholder="$t('buckets.addPlaceholder')"
        :aria-label="$t('buckets.addPlaceholder')"
        autocomplete="off"
        spellcheck="false"
      />
      <Button type="submit" variant="outline" :disabled="adding || !newBucket.trim()">
        <Plus /> {{ $t("buckets.add") }}
      </Button>
    </form>
  </section>
</template>
