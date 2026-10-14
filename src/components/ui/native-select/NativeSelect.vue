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
        'border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full rounded-md border bg-transparent px-2.5 text-sm shadow-xs outline-none focus-visible:ring-[3px] disabled:opacity-50 dark:bg-transparent [&>option]:bg-popover',
        props.class,
      )
    "
  >
    <slot />
  </select>
</template>
