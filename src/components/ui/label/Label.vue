<script setup lang="ts">
import { Label, type LabelProps } from "reka-ui";
import type { HTMLAttributes } from "vue";

import { cn } from "@/lib/utils";

const props = defineProps<LabelProps & { class?: HTMLAttributes["class"] }>();
</script>

<template>
  <Label :for="props.for" :class="cn('text-sm leading-none font-medium select-none', props.class)">
    <slot />
  </Label>
</template>
