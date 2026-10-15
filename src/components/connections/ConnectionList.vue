<script setup lang="ts">
import { Plus, Search } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";

import { useConnectionStore } from "@/stores/connections";

import ProviderMark from "./ProviderMark.vue";

const store = useConnectionStore();
const query = ref("");

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return store.connections;
  return store.connections.filter((c) => c.name.toLowerCase().includes(q));
});

onMounted(() => store.load());
</script>

<template>
  <div class="flex min-h-0 flex-col">
    <div class="flex items-center justify-between py-1 pr-1 pl-2">
      <span class="text-muted-foreground text-[11px] font-medium">
        {{ $t("nav.connections") }}
      </span>
      <button
        type="button"
        class="text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground inline-flex size-6 items-center justify-center rounded-md transition-colors active:scale-95"
        :title="$t('nav.addConnection')"
        :aria-label="$t('nav.addConnection')"
        @click="store.openCreateForm()"
      >
        <Plus class="size-4" />
      </button>
    </div>

    <label v-if="store.connections.length > 5" class="relative mx-1 mb-1.5 block">
      <Search class="text-muted-foreground absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
      <input
        v-model="query"
        type="search"
        :placeholder="$t('nav.search')"
        :aria-label="$t('nav.search')"
        class="bg-background/70 focus-visible:ring-ring/30 placeholder:text-muted-foreground/70 h-7 w-full rounded-md border pr-2 pl-8 text-xs outline-none focus-visible:ring-2"
      />
    </label>

    <p v-if="store.loadError" class="text-destructive px-2 py-1 text-xs" role="alert">
      {{ store.loadError.message }}
    </p>
    <template v-else-if="store.loading && !store.connections.length">
      <div v-for="n in 3" :key="n" class="flex items-center gap-2.5 px-2 py-1.5">
        <span class="skeleton size-7 rounded-md" />
        <span class="skeleton h-3 flex-1 rounded" />
      </div>
    </template>
    <button
      v-else-if="!store.connections.length"
      type="button"
      class="text-muted-foreground hover:text-foreground hover:border-ring/40 mx-1 mt-1 rounded-md border border-dashed px-3 py-3 text-left text-xs transition-colors"
      @click="store.openCreateForm()"
    >
      {{ $t("nav.noConnections") }}
      <span class="text-primary mt-0.5 block font-medium">{{ $t("nav.addConnection") }}</span>
    </button>
    <p v-else-if="!filtered.length" class="text-muted-foreground px-2 py-1 text-xs">
      {{ $t("nav.noMatches") }}
    </p>

    <ul class="flex flex-col gap-px">
      <li v-for="connection in filtered" :key="connection.id">
        <RouterLink
          :to="{ name: 'connection', params: { id: connection.id } }"
          class="group hover:bg-sidebar-accent/70 relative flex items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors"
          :class="{
            'bg-sidebar-accent text-sidebar-accent-foreground': $route.params.id === connection.id,
          }"
        >
          <span
            v-if="$route.params.id === connection.id"
            class="bg-primary absolute top-1.5 bottom-1.5 -left-2 w-0.5 rounded-full"
            aria-hidden="true"
          />
          <ProviderMark :provider="connection.provider" />
          <span class="min-w-0">
            <span class="block truncate text-[13px] leading-tight font-medium">
              {{ connection.name }}
            </span>
            <span class="text-muted-foreground block truncate text-[11px] leading-tight">
              {{ store.profileFor(connection.provider)?.label ?? connection.provider }}
            </span>
          </span>
        </RouterLink>
      </li>
    </ul>
  </div>
</template>
