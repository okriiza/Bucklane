<script setup lang="ts">
import { CircleAlert, Info, Plus, SlidersHorizontal, Trash2 } from "@lucide/vue";
import { computed, ref, watch } from "vue";

import SuggestInput from "@/components/common/SuggestInput.vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import Input from "@/components/ui/input/Input.vue";
import Label from "@/components/ui/label/Label.vue";
import { commands, toAppError, unwrap, type AppError, type ObjectMetadata } from "@/services/tauri";

const props = defineProps<{ connectionId: string; bucket: string; objectKey: string }>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ saved: [metadata: ObjectMetadata] }>();

const CONTENT_TYPES = [
  "text/html; charset=utf-8",
  "text/plain; charset=utf-8",
  "text/css; charset=utf-8",
  "text/csv; charset=utf-8",
  "application/javascript",
  "application/json",
  "application/pdf",
  "application/zip",
  "application/octet-stream",
  "image/png",
  "image/jpeg",
  "image/webp",
  "image/svg+xml",
  "video/mp4",
  "audio/mpeg",
];
const ENCODINGS = ["gzip", "br"];
const CACHE_CONTROLS = [
  "no-cache",
  "no-store",
  "public, max-age=3600",
  "public, max-age=86400",
  "public, max-age=31536000, immutable",
  "private, max-age=0",
];

interface Field {
  name: string;
  value: string;
}

const fileName = computed(() => props.objectKey.split("/").pop() ?? props.objectKey);

const loading = ref(false);
const busy = ref(false);
const error = ref<AppError | null>(null);
const etag = ref<string | null>(null);
const contentType = ref("");
const cacheControl = ref("");
const contentDisposition = ref("");
const contentEncoding = ref("");
const contentLanguage = ref("");
const fields = ref<Field[]>([]);

const dispositions = computed(() => [
  "inline",
  "attachment",
  `attachment; filename="${fileName.value.replace(/["\\]/g, "")}"`,
]);

// Always edit fresh values: the ETag guards against overwriting a newer file.
watch(open, async (isOpen) => {
  if (!isOpen) return;
  error.value = null;
  etag.value = null;
  loading.value = true;
  try {
    const current = await unwrap(
      commands.headObject(props.connectionId, props.bucket, props.objectKey),
    );
    etag.value = current.etag;
    contentType.value = current.contentType ?? "";
    cacheControl.value = current.cacheControl ?? "";
    contentDisposition.value = current.contentDisposition ?? "";
    contentEncoding.value = current.contentEncoding ?? "";
    contentLanguage.value = current.contentLanguage ?? "";
    fields.value = Object.entries(current.metadata).map(([name, value]) => ({ name, value }));
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    loading.value = false;
  }
});

function addField() {
  fields.value.push({ name: "", value: "" });
}

async function save() {
  if (!etag.value) return;
  busy.value = true;
  error.value = null;
  try {
    const metadata = Object.fromEntries(
      fields.value.filter((f) => f.name.trim() || f.value.trim()).map((f) => [f.name, f.value]),
    );
    const saved = await unwrap(
      commands.updateObjectMetadata({
        connectionId: props.connectionId,
        bucket: props.bucket,
        key: props.objectKey,
        etag: etag.value,
        headers: {
          contentType: contentType.value || null,
          cacheControl: cacheControl.value || null,
          contentDisposition: contentDisposition.value || null,
          contentEncoding: contentEncoding.value || null,
          contentLanguage: contentLanguage.value || null,
          metadata,
        },
      }),
    );
    emit("saved", saved);
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
    <DialogContent class="max-w-xl">
      <div class="space-y-1">
        <DialogTitle class="flex items-center gap-2 text-lg font-semibold">
          <SlidersHorizontal class="text-primary size-5" /> {{ $t("metadata.title") }}
        </DialogTitle>
        <DialogDescription class="text-muted-foreground text-[13px] break-all">
          {{ fileName }}
        </DialogDescription>
      </div>

      <p v-if="loading" class="text-muted-foreground text-[13px]">{{ $t("explorer.loading") }}</p>

      <form v-else-if="etag" class="grid gap-4" @submit.prevent="save">
        <div class="grid gap-3">
          <div class="grid gap-1.5">
            <Label for="meta-content-type">Content-Type</Label>
            <SuggestInput
              id="meta-content-type"
              v-model="contentType"
              :suggestions="CONTENT_TYPES"
              spellcheck="false"
              placeholder="application/octet-stream"
              class="font-mono text-xs"
            />
          </div>
          <div class="grid gap-1.5">
            <Label for="meta-cache-control">Cache-Control</Label>
            <SuggestInput
              id="meta-cache-control"
              v-model="cacheControl"
              :suggestions="CACHE_CONTROLS"
              spellcheck="false"
              placeholder="public, max-age=3600"
              class="font-mono text-xs"
            />
          </div>
          <div class="grid gap-1.5">
            <Label for="meta-content-disposition">Content-Disposition</Label>
            <SuggestInput
              id="meta-content-disposition"
              v-model="contentDisposition"
              :suggestions="dispositions"
              spellcheck="false"
              placeholder="inline"
              class="font-mono text-xs"
            />
          </div>
          <div class="grid gap-3 sm:grid-cols-2">
            <div class="grid gap-1.5">
              <Label for="meta-content-encoding">Content-Encoding</Label>
              <SuggestInput
                id="meta-content-encoding"
                v-model="contentEncoding"
                :suggestions="ENCODINGS"
                spellcheck="false"
                placeholder="gzip"
                class="font-mono text-xs"
              />
            </div>
            <div class="grid gap-1.5">
              <Label for="meta-content-language">Content-Language</Label>
              <Input
                id="meta-content-language"
                v-model="contentLanguage"
                autocomplete="off"
                spellcheck="false"
                placeholder="en"
                class="font-mono text-xs"
              />
            </div>
          </div>
        </div>

        <fieldset class="grid gap-2">
          <legend class="mb-2 text-[13px] font-medium">{{ $t("explorer.userMetadata") }}</legend>
          <p v-if="!fields.length" class="text-muted-foreground text-xs">
            {{ $t("metadata.noCustom") }}
          </p>
          <div v-for="(field, index) in fields" :key="index" class="flex items-center gap-2">
            <Input
              v-model="field.name"
              :aria-label="$t('metadata.name')"
              :placeholder="$t('metadata.name')"
              autocomplete="off"
              spellcheck="false"
              class="w-2/5 font-mono text-xs"
            />
            <Input
              v-model="field.value"
              :aria-label="$t('metadata.value')"
              :placeholder="$t('metadata.value')"
              autocomplete="off"
              spellcheck="false"
              class="flex-1 font-mono text-xs"
            />
            <Button
              variant="ghost"
              size="icon"
              type="button"
              :aria-label="$t('metadata.remove')"
              :title="$t('metadata.remove')"
              @click="fields.splice(index, 1)"
            >
              <Trash2 />
            </Button>
          </div>
          <div>
            <Button variant="outline" size="sm" type="button" @click="addField">
              <Plus /> {{ $t("metadata.add") }}
            </Button>
          </div>
        </fieldset>

        <p class="text-muted-foreground flex gap-2 text-xs">
          <Info class="mt-px size-4 shrink-0" />
          {{ $t("metadata.note") }}
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
          <Button variant="ghost" type="button" :disabled="busy" @click="open = false">
            {{ $t("common.cancel") }}
          </Button>
          <Button type="submit" :disabled="busy">
            {{ busy ? $t("metadata.saving") : $t("metadata.save") }}
          </Button>
        </div>
      </form>

      <template v-else>
        <p
          v-if="error"
          class="border-destructive/30 bg-destructive/8 text-destructive flex gap-2 rounded-lg border p-3 text-[13px]"
          role="alert"
        >
          <CircleAlert class="mt-0.5 size-4 shrink-0" />
          {{ error.message }}
        </p>
        <div class="flex justify-end">
          <Button variant="ghost" @click="open = false">{{ $t("common.close") }}</Button>
        </div>
      </template>
    </DialogContent>
  </Dialog>
</template>
