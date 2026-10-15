<script setup lang="ts">
import { X } from "@lucide/vue";
import { DialogClose, DialogContent, DialogOverlay, DialogPortal } from "reka-ui";
import type { HTMLAttributes } from "vue";

import { cn } from "@/lib/utils";

const props = defineProps<{ class?: HTMLAttributes["class"] }>();
</script>

<template>
  <DialogPortal>
    <DialogOverlay
      class="data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 fixed inset-0 z-50 bg-black/40 backdrop-blur-[2px]"
    />
    <DialogContent
      :class="
        cn(
          'bg-popover text-popover-foreground shadow-[var(--shadow-float)] data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 fixed top-1/2 left-1/2 z-50 grid max-h-[calc(100%-2rem)] w-full max-w-lg -translate-x-1/2 -translate-y-1/2 gap-4 overflow-y-auto rounded-xl border p-6 duration-200',
          props.class,
        )
      "
    >
      <slot />
      <DialogClose
        class="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/40 absolute top-3.5 right-3.5 rounded-md p-1 transition-colors focus:outline-none focus-visible:ring-2"
      >
        <X class="size-4" />
        <span class="sr-only">Close</span>
      </DialogClose>
    </DialogContent>
  </DialogPortal>
</template>
