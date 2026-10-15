<script setup lang="ts">
import type { HTMLAttributes } from "vue";

import { cn } from "@/lib/utils";

const props = defineProps<{ class?: HTMLAttributes["class"] }>();
const model = defineModel<string>({ default: "" });
</script>

<template>
  <input
    v-model="model"
    :class="
      cn(
        'border-input bg-background placeholder:text-muted-foreground/70 focus-visible:border-ring focus-visible:ring-ring/25 aria-invalid:border-destructive dark:bg-input/30 flex h-8 w-full min-w-0 rounded-md border px-2.5 text-[13px] shadow-[var(--shadow-soft)] transition-[border-color,box-shadow] outline-none select-text focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50',
        props.class,
      )
    "
  />
</template>
