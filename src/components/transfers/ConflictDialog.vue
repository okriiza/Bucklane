<script setup lang="ts">
import { computed } from "vue";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { useConflictPromptStore } from "@/stores/conflictPrompt";

const prompt = useConflictPromptStore();

const open = computed({
  get: () => prompt.open,
  set: (value) => {
    if (!value) prompt.answer(null);
  },
});
</script>

<!-- One decision applies to every conflicting item in the batch (TSD §6.5.1). -->
<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md">
      <DialogTitle class="text-lg font-semibold">
        {{ $t("conflict.title", { count: prompt.names.length }, prompt.names.length) }}
      </DialogTitle>
      <DialogDescription class="text-muted-foreground text-sm">
        {{ $t(`conflict.description.${prompt.direction}`) }}
      </DialogDescription>
      <ul class="max-h-40 overflow-auto rounded-md border text-xs">
        <li
          v-for="name in prompt.names"
          :key="name"
          class="border-b px-3 py-1.5 font-mono break-all last:border-b-0"
        >
          {{ name }}
        </li>
      </ul>
      <div class="flex flex-wrap justify-end gap-2">
        <Button variant="ghost" @click="prompt.answer(null)">{{ $t("common.cancel") }}</Button>
        <Button variant="outline" @click="prompt.answer('SKIP')">{{ $t("conflict.skip") }}</Button>
        <Button variant="outline" @click="prompt.answer('RENAME')">
          {{ $t("conflict.keepBoth") }}
        </Button>
        <Button variant="destructive" @click="prompt.answer('OVERWRITE')">
          {{ $t("conflict.replace") }}
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
