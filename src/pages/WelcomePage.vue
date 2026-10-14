<script setup lang="ts">
import { Plus } from "@lucide/vue";

import { Button } from "@/components/ui/button";
import { useConnectionStore } from "@/stores/connections";

const store = useConnectionStore();
</script>

<template>
  <div class="flex h-full items-center justify-center p-8">
    <div class="max-w-md text-center">
      <h1 class="text-xl font-semibold tracking-tight">{{ $t("welcome.title") }}</h1>
      <p class="text-muted-foreground mt-2 text-sm leading-relaxed">
        {{ $t("welcome.description") }}
      </p>
      <Button class="mt-6" @click="store.openCreateForm()">
        <Plus /> {{ $t("welcome.addConnection") }}
      </Button>
    </div>
  </div>
</template>
