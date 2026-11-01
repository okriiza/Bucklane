<script setup lang="ts">
import { PopoverAnchor, PopoverContent, PopoverPortal, PopoverRoot } from "reka-ui";
import { computed, ref, useId, watch, type HTMLAttributes } from "vue";

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

const matches = computed(() => {
  const query = model.value.trim().toLowerCase();
  return props.suggestions.filter((s) => s !== model.value && s.toLowerCase().includes(query));
});
const visible = computed(() => open.value && matches.value.length > 0);

watch(matches, () => (highlighted.value = -1));

function pick(value: string) {
  model.value = value;
  open.value = false;
}

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
        @focus="open = true"
        @click="open = true"
        @input="open = true"
        @blur="open = false"
        @keydown="onKeydown"
      />
    </PopoverAnchor>
    <PopoverPortal>
      <PopoverContent
        :id="listId"
        role="listbox"
        align="start"
        :side-offset="4"
        class="bg-popover text-popover-foreground z-50 max-h-56 w-(--reka-popover-trigger-width) overflow-auto rounded-lg border p-1 shadow-[var(--shadow-float)]"
        @open-auto-focus.prevent
        @close-auto-focus.prevent
        @interact-outside="(event) => isInput(event.target) && event.preventDefault()"
      >
        <div
          v-for="(value, index) in matches"
          :id="`${listId}-${index}`"
          :key="value"
          role="option"
          :aria-selected="index === highlighted"
          class="cursor-default truncate rounded-md px-2 py-1.5 font-mono text-xs select-none"
          :class="index === highlighted ? 'bg-accent' : 'hover:bg-accent'"
          @mousedown.prevent="pick(value)"
          @mousemove="highlighted = index"
        >
          {{ value }}
        </div>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
