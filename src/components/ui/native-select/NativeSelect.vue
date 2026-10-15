<script setup lang="ts" generic="T extends string">
import type { HTMLAttributes } from "vue";

import { cn } from "@/lib/utils";

const props = defineProps<{ class?: HTMLAttributes["class"] }>();
const model = defineModel<T>({ required: true });
</script>

<!-- Native <select>: platform-consistent keyboard and accessibility behavior. -->
<template>
  <select
    v-model="model"
    :class="
      cn(
        'border-input bg-background focus-visible:border-ring focus-visible:ring-ring/25 dark:bg-input/30 h-8 w-full rounded-md border px-2 text-[13px] shadow-[var(--shadow-soft)] outline-none focus-visible:ring-[3px] disabled:opacity-50 [&>option]:bg-popover',
        props.class,
      )
    "
  >
    <slot />
  </select>
</template>
