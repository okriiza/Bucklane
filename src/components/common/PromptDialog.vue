<script setup lang="ts">
import { CircleAlert } from "@lucide/vue";
import { nextTick, ref, watch } from "vue";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import Input from "@/components/ui/input/Input.vue";
import Label from "@/components/ui/label/Label.vue";
import { toAppError, type AppError } from "@/services/tauri";

const props = defineProps<{
  title: string;
  description?: string;
  label: string;
  initialValue: string;
  confirmLabel: string;
  /** Selects this range of the initial value, e.g. a file name without its extension. */
  selectRange?: [number, number];
  /** Returns normally on success; a thrown error is shown inline. */
  submit: (value: string) => Promise<void>;
}>();
const open = defineModel<boolean>("open", { required: true });

const value = ref("");
const busy = ref(false);
const error = ref<AppError | null>(null);
const inputId = `prompt-${Math.random().toString(36).slice(2)}`;

watch(open, async (isOpen) => {
  if (!isOpen) return;
  value.value = props.initialValue;
  error.value = null;
  await nextTick();
  const input = document.getElementById(inputId) as HTMLInputElement | null;
  input?.focus();
  if (props.selectRange) input?.setSelectionRange(...props.selectRange);
  else input?.select();
});

async function onSubmit() {
  busy.value = true;
  error.value = null;
  try {
    await props.submit(value.value);
    open.value = false;
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md">
      <div class="space-y-1">
        <DialogTitle class="text-lg font-semibold">{{ title }}</DialogTitle>
        <DialogDescription v-if="description" class="text-muted-foreground text-sm break-all">
          {{ description }}
        </DialogDescription>
      </div>
      <form class="grid gap-4" @submit.prevent="onSubmit">
        <div class="grid gap-2">
          <Label :for="inputId">{{ label }}</Label>
          <Input :id="inputId" v-model="value" autocomplete="off" spellcheck="false" />
        </div>
        <p
          v-if="error"
          class="border-destructive/40 bg-destructive/10 text-destructive flex gap-2 rounded-md border p-3 text-sm"
          role="alert"
        >
          <CircleAlert class="mt-0.5 size-4 shrink-0" />
          {{ error.message }}
        </p>
        <div class="flex justify-end gap-2">
          <Button variant="ghost" :disabled="busy" @click="open = false">
            {{ $t("common.cancel") }}
          </Button>
          <Button type="submit" :disabled="busy || !value.trim()">{{ confirmLabel }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
