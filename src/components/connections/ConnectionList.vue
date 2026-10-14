<script setup lang="ts">
import { Plus, Search } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";

import { Button } from "@/components/ui/button";
import { useConnectionStore } from "@/stores/connections";

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
    <div class="flex items-center justify-between px-2 pt-2 pb-1">
      <span class="text-muted-foreground text-xs font-medium tracking-wide uppercase">
        {{ $t("nav.connections") }}
      </span>
      <Button
        variant="ghost"
        size="icon"
        class="size-6"
        :title="$t('nav.addConnection')"
        :aria-label="$t('nav.addConnection')"
        @click="store.openCreateForm()"
      >
        <Plus />
      </Button>
    </div>

    <label v-if="store.connections.length > 5" class="relative mx-2 mb-1 block">
      <Search class="text-muted-foreground absolute top-1/2 left-2 size-3.5 -translate-y-1/2" />
      <input
        v-model="query"
        type="search"
        :placeholder="$t('nav.search')"
        :aria-label="$t('nav.search')"
        class="bg-background focus-visible:ring-ring/50 h-7 w-full rounded-md border pr-2 pl-7 text-xs outline-none focus-visible:ring-2"
      />
    </label>

    <p v-if="store.loadError" class="text-destructive px-2 py-1 text-sm" role="alert">
      {{ store.loadError.message }}
    </p>
    <p v-else-if="!store.connections.length" class="text-muted-foreground px-2 py-1 text-sm">
      {{ $t("nav.noConnections") }}
    </p>
    <p v-else-if="!filtered.length" class="text-muted-foreground px-2 py-1 text-sm">
      {{ $t("nav.noMatches") }}
    </p>

    <ul class="flex flex-col gap-0.5">
      <li v-for="connection in filtered" :key="connection.id">
        <RouterLink
          :to="{ name: 'connection', params: { id: connection.id } }"
          class="hover:bg-sidebar-accent flex flex-col rounded-md px-2 py-1.5"
          active-class="bg-sidebar-accent"
        >
          <span class="truncate text-sm font-medium">{{ connection.name }}</span>
          <span class="text-muted-foreground truncate text-xs">
            {{ store.profileFor(connection.provider)?.label ?? connection.provider }}
          </span>
        </RouterLink>
      </li>
    </ul>
  </div>
</template>
