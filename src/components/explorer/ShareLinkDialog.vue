<script setup lang="ts">
import { Check, CircleAlert, Copy, Link2, TriangleAlert } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import Label from "@/components/ui/label/Label.vue";
import NativeSelect from "@/components/ui/native-select/NativeSelect.vue";
import { commands, toAppError, unwrap, type AppError, type ShareLink } from "@/services/tauri";
import { copyText } from "@/utils/clipboard";
import { formatDateTime } from "@/utils/format";

const props = defineProps<{
  connectionId: string;
  bucket: string;
  objectKey: string;
  insecure: boolean;
}>();
const open = defineModel<boolean>("open", { required: true });
const { t } = useI18n();

const DURATIONS = [
  { value: "900", label: "share.durations.15m" },
  { value: "3600", label: "share.durations.1h" },
  { value: "86400", label: "share.durations.1d" },
  { value: "604800", label: "share.durations.7d" },
] as const;

const duration = ref<string>("3600");
const link = ref<ShareLink | null>(null);
const error = ref<AppError | null>(null);
const busy = ref(false);
const copied = ref(false);

const fileName = computed(() => props.objectKey.split("/").pop() ?? props.objectKey);

watch(open, (isOpen) => {
  if (!isOpen) return;
  link.value = null;
  error.value = null;
  copied.value = false;
});

// A different duration needs a new signature.
watch(duration, () => {
  link.value = null;
  copied.value = false;
});

async function generate() {
  busy.value = true;
  error.value = null;
  try {
    link.value = await unwrap(
      commands.createShareLink({
        connectionId: props.connectionId,
        bucket: props.bucket,
        key: props.objectKey,
        expiresInSeconds: Number(duration.value),
      }),
    );
    await copy();
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    busy.value = false;
  }
}

async function copy() {
  if (!link.value) return;
  try {
    await copyText(link.value.url);
    copied.value = true;
    setTimeout(() => (copied.value = false), 2000);
  } catch {
    error.value = {
      code: "INTERNAL_ERROR",
      message: t("share.copyFailed"),
      details: null,
      retryable: false,
    };
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-lg">
      <div class="space-y-1">
        <DialogTitle class="flex items-center gap-2 text-lg font-semibold">
          <Link2 class="text-primary size-5" /> {{ $t("share.title") }}
        </DialogTitle>
        <DialogDescription class="text-muted-foreground text-[13px] break-all">
          {{ fileName }}
        </DialogDescription>
      </div>

      <div class="grid gap-2">
        <Label for="share-duration">{{ $t("share.expiresIn") }}</Label>
        <NativeSelect id="share-duration" v-model="duration" class="max-w-48">
          <option v-for="d in DURATIONS" :key="d.value" :value="d.value">{{ $t(d.label) }}</option>
        </NativeSelect>
      </div>

      <div v-if="link" class="grid gap-2">
        <div class="flex gap-2">
          <input
            :value="link.url"
            readonly
            :aria-label="$t('share.title')"
            class="border-input bg-muted/50 h-8 min-w-0 flex-1 rounded-md border px-2.5 font-mono text-xs outline-none select-all"
            @focus="($event.target as HTMLInputElement).select()"
          />
          <Button variant="outline" @click="copy">
            <Check v-if="copied" class="text-success" />
            <Copy v-else />
            {{ copied ? $t("share.copied") : $t("share.copy") }}
          </Button>
        </div>
        <p class="text-muted-foreground text-xs">
          {{ $t("share.expiresAt", { time: formatDateTime(link.expiresAt) }) }}
        </p>
      </div>

      <p
        class="flex gap-2 rounded-lg border border-amber-500/30 bg-amber-500/8 p-3 text-xs text-amber-700 dark:text-amber-300"
      >
        <TriangleAlert class="mt-px size-4 shrink-0" />
        <span>
          {{ $t("share.warning") }}
          <template v-if="insecure"> {{ $t("share.insecure") }}</template>
        </span>
      </p>

      <p
        v-if="error"
        class="border-destructive/30 bg-destructive/8 text-destructive flex gap-2 rounded-lg border p-3 text-[13px]"
        role="alert"
      >
        <CircleAlert class="mt-0.5 size-4 shrink-0" />
        {{ error.message }}
      </p>

      <div class="flex justify-end gap-2">
        <Button variant="ghost" @click="open = false">{{ $t("common.close") }}</Button>
        <Button :disabled="busy" @click="generate">
          <Link2 />
          {{ busy ? $t("share.creating") : link ? $t("share.regenerate") : $t("share.create") }}
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
