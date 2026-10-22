<script setup lang="ts">
import { File, Folder } from "@lucide/vue";
import { computed } from "vue";

import { CANCEL_MS, LAND_MS, type DragState } from "@/composables/useObjectDrag";

const props = defineProps<{
  drag: DragState;
  /** Main line, e.g. the folder name or "3 files". */
  label: string;
  /** Second line, e.g. "Move into exports"; shown while over a target. */
  hint: string | null;
}>();

const isFolder = computed(() => props.drag.keys[0]?.endsWith("/") ?? false);
const stacked = computed(() => props.drag.keys.length > 1);

const style = computed(() => {
  const { x, y, phase } = props.drag;
  if (phase === "landing") {
    // Shrink into the target folder's icon.
    return {
      transform: `translate(${x - 18}px, ${y - 18}px) scale(0.2)`,
      opacity: "0",
      transition: `transform ${LAND_MS}ms cubic-bezier(0.4, 0, 0.2, 1), opacity ${LAND_MS}ms ease-in`,
    };
  }
  if (phase === "canceled") {
    return {
      transform: `translate(${x + 14}px, ${y + 14}px) scale(0.92)`,
      opacity: "0",
      transition: `transform ${CANCEL_MS}ms ease-out, opacity ${CANCEL_MS}ms ease-out`,
    };
  }
  return { transform: `translate(${x + 14}px, ${y + 14}px)` };
});
</script>

<template>
  <div
    class="pointer-events-none fixed top-0 left-0 z-50 origin-top-left will-change-transform"
    :style="style"
    aria-hidden="true"
  >
    <div class="drag-ghost-in relative">
      <!-- A second card peeks out behind when several items are dragged. -->
      <div
        v-if="stacked"
        class="bg-popover absolute inset-0 translate-x-1.5 translate-y-1.5 rotate-3 rounded-lg border opacity-70"
      />
      <div
        class="bg-popover text-popover-foreground relative flex max-w-64 items-center gap-2.5 rounded-lg border px-3 py-2 shadow-[var(--shadow-float)]"
        :class="{ 'ring-primary/60 ring-2': hint }"
      >
        <component
          :is="isFolder ? Folder : File"
          class="size-5 shrink-0"
          :class="isFolder ? 'text-primary fill-primary/15' : 'text-muted-foreground'"
        />
        <div class="min-w-0">
          <div class="truncate text-[13px] font-medium">{{ label }}</div>
          <div v-if="hint" class="text-primary truncate text-[11px]">{{ hint }}</div>
        </div>
        <span
          v-if="stacked"
          class="bg-primary text-primary-foreground tabular absolute -top-2 -right-2 grid h-5 min-w-5 place-items-center rounded-full px-1 text-[11px] font-semibold"
        >
          {{ drag.keys.length }}
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Lift off the row when the drag starts. */
.drag-ghost-in {
  animation: ghost-in 140ms cubic-bezier(0.2, 0, 0, 1);
  transform: rotate(-1.5deg);
}

@keyframes ghost-in {
  from {
    opacity: 0;
    transform: scale(0.9) rotate(0deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .drag-ghost-in {
    animation: none;
  }
}
</style>
