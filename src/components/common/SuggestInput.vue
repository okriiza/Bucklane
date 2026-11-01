<script setup lang="ts">
import { Check } from "@lucide/vue";
import { PopoverAnchor, PopoverContent, PopoverPortal, PopoverRoot } from "reka-ui";
import { computed, nextTick, onBeforeUnmount, ref, useId, watch, type HTMLAttributes } from "vue";

import Input from "@/components/ui/input/Input.vue";

/**
 * A text field with suggestions that closes once a suggestion is picked.
 * Replaces `<datalist>`, which WebKit keeps open while the value matches.
 * Any typed value is allowed.
 */
const props = defineProps<{
  suggestions: string[];
  class?: HTMLAttributes["class"];
}>();
const model = defineModel<string>({ default: "" });
defineOptions({ inheritAttrs: false });

const open = ref(false);
const highlighted = ref(-1);
const listId = useId();
const input = ref<InstanceType<typeof Input> | null>(null);

// The whole list while the value is one of the suggestions (just picked or
// prefilled), so another can be chosen; otherwise the matches.
const matches = computed(() => {
  const query = model.value.trim().toLowerCase();
  if (!query || props.suggestions.includes(model.value)) return props.suggestions;
  return props.suggestions.filter((s) => s.toLowerCase().includes(query));
});
const list = ref<HTMLElement | null>(null);
const scrolled = ref({ top: false, bottom: false });

/** Which edges have more items beyond them, for the fade hints. */
function measure() {
  const el = list.value;
  if (!el) return;
  scrolled.value = {
    top: el.scrollTop > 0,
    bottom: el.scrollTop + el.clientHeight < el.scrollHeight - 1,
  };
}
const visible = computed(() => open.value && matches.value.length > 0);

watch(matches, () => (highlighted.value = -1));

function pick(value: string) {
  model.value = value;
  open.value = false;
}

// The list's height settles only after the popover is positioned.
const resizes = new ResizeObserver(() => measure());
watch(list, (el, previous) => {
  if (previous) resizes.unobserve(previous);
  if (el) resizes.observe(el);
});
onBeforeUnmount(() => resizes.disconnect());

// Opening highlights the current value, if it is a suggestion.
watch(visible, async (isVisible) => {
  if (!isVisible) return;
  highlighted.value = matches.value.indexOf(model.value);
  await nextTick();
  measure();
});

// Keep the highlighted row in view while moving with the arrow keys.
watch(highlighted, async (index) => {
  await nextTick();
  if (index >= 0)
    document.getElementById(`${listId}-${index}`)?.scrollIntoView({ block: "nearest" });
  measure();
});

function onKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    if (!visible.value) {
      open.value = true;
      return;
    }
    const step = event.key === "ArrowDown" ? 1 : -1;
    const count = matches.value.length;
    highlighted.value = (highlighted.value + step + count) % count;
  } else if (event.key === "Enter" && visible.value && highlighted.value >= 0) {
    // Picking a suggestion must not submit the form.
    event.preventDefault();
    pick(matches.value[highlighted.value] ?? model.value);
  } else if (event.key === "Escape" && visible.value) {
    // Close the list, not the dialog around it.
    event.preventDefault();
    event.stopPropagation();
    open.value = false;
  }
}

function isInput(target: EventTarget | null) {
  const el = (input.value?.$el as HTMLElement | undefined) ?? null;
  return !!el && target instanceof Node && el.contains(target);
}
</script>

<template>
  <PopoverRoot :open="visible" @update:open="(value) => !value && (open = false)">
    <PopoverAnchor as-child>
      <Input
        ref="input"
        v-bind="$attrs"
        v-model="model"
        :class="props.class"
        role="combobox"
        aria-autocomplete="list"
        :aria-expanded="visible"
        :aria-controls="listId"
        :aria-activedescendant="highlighted >= 0 ? `${listId}-${highlighted}` : undefined"
        autocomplete="off"
        @click="open = true"
        @input="open = true"
        @blur="open = false"
        @keydown="onKeydown"
      />
    </PopoverAnchor>
    <PopoverPortal>
      <PopoverContent
        align="start"
        :side-offset="4"
        :collision-padding="12"
        class="bg-popover text-popover-foreground relative z-50 w-(--reka-popover-trigger-width) overflow-hidden rounded-lg border shadow-[var(--shadow-float)]"
        @open-auto-focus.prevent
        @close-auto-focus.prevent
        @interact-outside="(event) => isInput(event.target) && event.preventDefault()"
      >
        <div
          :id="listId"
          ref="list"
          role="listbox"
          class="max-h-[min(13.5rem,calc(var(--reka-popover-content-available-height)-0.5rem))] scroll-py-5 overflow-auto p-1"
          @scroll="measure"
        >
          <div
            v-for="(value, index) in matches"
            :id="`${listId}-${index}`"
            :key="value"
            role="option"
            :aria-selected="value === model"
            class="flex cursor-default items-center gap-2 rounded-md px-2 py-1 font-mono text-xs select-none"
            :class="index === highlighted ? 'bg-accent' : ''"
            @mousedown.prevent="pick(value)"
            @mousemove="highlighted = index"
          >
            <span class="min-w-0 flex-1 truncate">{{ value }}</span>
            <Check v-if="value === model" class="text-primary size-3.5 shrink-0" />
          </div>
        </div>
        <!-- Fades hint that the list continues. -->
        <div
          v-if="scrolled.top"
          class="from-popover pointer-events-none absolute inset-x-0 top-0 h-5 bg-gradient-to-b"
        />
        <div
          v-if="scrolled.bottom"
          class="from-popover pointer-events-none absolute inset-x-0 bottom-0 h-5 bg-gradient-to-t"
        />
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
