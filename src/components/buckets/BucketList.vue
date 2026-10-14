<script setup lang="ts">
import { Database, Plus, RefreshCw, Star } from "@lucide/vue";
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
    <header class="mb-3 flex items-center justify-between">
      <h2 class="text-sm font-semibold">{{ $t("buckets.title") }}</h2>
      <Button
        variant="ghost"
        size="icon"
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
      class="text-muted-foreground mb-3 rounded-md border border-dashed p-3 text-sm"
      role="note"
    >
      <template v-if="store.listError.code === 'ACCESS_DENIED'">
        {{ $t("buckets.listDenied") }}
      </template>
      <template v-else>{{ store.listError.message }}</template>
    </p>

    <p
      v-if="!store.loading && !store.entries.length && !store.listError"
      class="text-muted-foreground mb-3 text-sm"
    >
      {{ $t("buckets.empty") }}
    </p>

    <ul class="divide-y rounded-md border" :class="{ hidden: !store.entries.length }">
      <li v-for="entry in store.entries" :key="entry.name" class="flex items-center gap-1 pr-1">
        <RouterLink
          :to="{ name: 'explorer', params: { id: connectionId, bucket: entry.name } }"
          class="hover:bg-accent flex min-w-0 flex-1 items-center gap-3 px-3 py-2"
        >
          <Database class="text-muted-foreground size-4 shrink-0" />
          <span class="truncate text-sm">{{ entry.label }}</span>
          <span
            v-if="entry.label !== entry.name"
            class="text-muted-foreground truncate font-mono text-xs"
          >
            {{ entry.name }}
          </span>
          <span v-if="entry.region" class="text-muted-foreground ml-auto font-mono text-xs">
            {{ entry.region }}
          </span>
        </RouterLink>
        <Button
          variant="ghost"
          size="icon"
          :title="bookmarkLabel(entry)"
          :aria-label="bookmarkLabel(entry)"
          :aria-pressed="!!entry.bookmark"
          @click="toggleBookmark(entry)"
        >
          <Star
            :class="entry.bookmark ? 'fill-amber-400 text-amber-400' : 'text-muted-foreground'"
          />
        </Button>
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
