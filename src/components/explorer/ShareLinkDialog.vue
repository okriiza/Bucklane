<script setup lang="ts">
import { Check, CircleAlert, Copy, Globe, Info, Link2, TriangleAlert } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import Input from "@/components/ui/input/Input.vue";
import Label from "@/components/ui/label/Label.vue";
import NativeSelect from "@/components/ui/native-select/NativeSelect.vue";
import {
  commands,
  toAppError,
  unwrap,
  type AppError,
  type PublicUrl,
  type ShareLink,
} from "@/services/tauri";
import { copyText } from "@/utils/clipboard";
import { formatDateTime } from "@/utils/format";

const props = defineProps<{
  connectionId: string;
  bucket: string;
  objectKey: string;
  insecure: boolean;
  /** Tab shown when the dialog opens. */
  initialMode?: Mode;
}>();
const open = defineModel<boolean>("open", { required: true });

type Mode = "temporary" | "public";
const mode = ref<Mode>("temporary");
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

const publicUrl = ref<PublicUrl | null>(null);
const publicLoading = ref(false);
const baseUrl = ref("");
const savingBase = ref(false);

const fileName = computed(() => props.objectKey.split("/").pop() ?? props.objectKey);

watch(open, (isOpen) => {
  if (!isOpen) return;
  link.value = null;
  publicUrl.value = null;
  error.value = null;
  copied.value = false;
  mode.value = props.initialMode ?? "temporary";
  if (mode.value === "public") void loadPublicUrl();
});

watch(mode, (next) => {
  error.value = null;
  copied.value = false;
  if (next === "public" && !publicUrl.value) void loadPublicUrl();
});

async function loadPublicUrl() {
  publicLoading.value = true;
  try {
    publicUrl.value = await unwrap(
      commands.getPublicUrl(props.connectionId, props.bucket, props.objectKey),
    );
    baseUrl.value = publicUrl.value.baseUrl ?? "";
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    publicLoading.value = false;
  }
}

async function saveBaseUrl(value: string | null) {
  savingBase.value = true;
  error.value = null;
  try {
    await unwrap(
      commands.setBucketPublicUrl({
        connectionId: props.connectionId,
        bucket: props.bucket,
        baseUrl: value,
      }),
    );
    copied.value = false;
    await loadPublicUrl();
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    savingBase.value = false;
  }
}

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
  const url = mode.value === "public" ? publicUrl.value?.url : link.value?.url;
  if (!url) return;
  try {
    await copyText(url);
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

      <div
        role="tablist"
        :aria-label="$t('share.title')"
        class="bg-muted grid grid-cols-2 gap-1 rounded-lg p-1 text-[13px]"
      >
        <button
          v-for="tab in ['temporary', 'public'] as const"
          :key="tab"
          type="button"
          role="tab"
          :aria-selected="mode === tab"
          class="flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition-colors [&_svg]:size-4"
          :class="
            mode === tab
              ? 'bg-background text-foreground shadow-[var(--shadow-soft)]'
              : 'text-muted-foreground hover:text-foreground'
          "
          @click="mode = tab"
        >
          <Link2 v-if="tab === 'temporary'" />
          <Globe v-else />
          {{ $t(`share.modes.${tab}`) }}
        </button>
      </div>

      <template v-if="mode === 'temporary'">
        <div class="grid gap-2">
          <Label for="share-duration">{{ $t("share.expiresIn") }}</Label>
          <NativeSelect id="share-duration" v-model="duration" class="max-w-48">
            <option v-for="d in DURATIONS" :key="d.value" :value="d.value">
              {{ $t(d.label) }}
            </option>
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
      </template>

      <template v-else>
        <p v-if="publicLoading && !publicUrl" class="text-muted-foreground text-[13px]">
          {{ $t("explorer.loading") }}
        </p>
        <div v-else-if="publicUrl?.url" class="flex gap-2">
          <input
            :value="publicUrl.url"
            readonly
            :aria-label="$t('share.modes.public')"
            class="border-input bg-muted/50 h-8 min-w-0 flex-1 rounded-md border px-2.5 font-mono text-xs outline-none select-all"
            @focus="($event.target as HTMLInputElement).select()"
          />
          <Button variant="outline" @click="copy">
            <Check v-if="copied" class="text-success" />
            <Copy v-else />
            {{ copied ? $t("share.copied") : $t("share.copy") }}
          </Button>
        </div>
        <p v-else-if="publicUrl" class="text-muted-foreground text-[13px]">
          {{ $t("share.publicNeedsBase") }}
        </p>

        <form class="grid gap-2" @submit.prevent="saveBaseUrl(baseUrl)">
          <Label for="share-base-url">{{ $t("share.baseUrl") }}</Label>
          <div class="flex gap-2">
            <Input
              id="share-base-url"
              v-model="baseUrl"
              autocomplete="off"
              spellcheck="false"
              placeholder="https://cdn.example.com"
              class="font-mono text-xs"
            />
            <Button
              type="submit"
              variant="outline"
              :disabled="savingBase || !baseUrl.trim() || baseUrl.trim() === publicUrl?.baseUrl"
            >
              {{ $t("common.save") }}
            </Button>
            <Button
              v-if="publicUrl?.baseUrl"
              type="button"
              variant="ghost"
              :disabled="savingBase"
              @click="saveBaseUrl(null)"
            >
              {{ $t("share.removeBase") }}
            </Button>
          </div>
          <p class="text-muted-foreground text-xs">
            {{ $t("share.baseUrlHint", { bucket }) }}
          </p>
        </form>

        <p class="text-muted-foreground flex gap-2 text-xs">
          <Info class="mt-px size-4 shrink-0" />
          {{ $t("share.publicNote") }}
        </p>
      </template>

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
        <Button v-if="mode === 'temporary'" :disabled="busy" @click="generate">
          <Link2 />
          {{ busy ? $t("share.creating") : link ? $t("share.regenerate") : $t("share.create") }}
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
