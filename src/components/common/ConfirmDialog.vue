<script setup lang="ts">
import {
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogRoot,
  AlertDialogTitle,
} from "reka-ui";

import { buttonVariants } from "@/components/ui/button";

defineProps<{
  title: string;
  description: string;
  confirmLabel: string;
  destructive?: boolean;
}>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ confirm: [] }>();
</script>

<!-- Confirmation for destructive actions (SEC-005). Cancel is focused by default. -->
<template>
  <AlertDialogRoot v-model:open="open">
    <AlertDialogPortal>
      <AlertDialogOverlay
        class="data-[state=open]:animate-in data-[state=open]:fade-in-0 fixed inset-0 z-50 bg-black/40 backdrop-blur-[2px]"
      />
      <AlertDialogContent
        class="bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 fixed top-1/2 left-1/2 z-50 grid w-full max-w-md -translate-x-1/2 -translate-y-1/2 gap-4 rounded-xl border p-6 shadow-[var(--shadow-float)]"
      >
        <AlertDialogTitle class="text-lg font-semibold">{{ title }}</AlertDialogTitle>
        <AlertDialogDescription class="text-muted-foreground text-sm">
          {{ description }}
        </AlertDialogDescription>
        <div class="flex justify-end gap-2">
          <AlertDialogCancel :class="buttonVariants({ variant: 'outline' })">
            {{ $t("common.cancel") }}
          </AlertDialogCancel>
          <AlertDialogAction
            :class="buttonVariants({ variant: destructive ? 'destructive' : 'default' })"
            @click="emit('confirm')"
          >
            {{ confirmLabel }}
          </AlertDialogAction>
        </div>
      </AlertDialogContent>
    </AlertDialogPortal>
  </AlertDialogRoot>
</template>
