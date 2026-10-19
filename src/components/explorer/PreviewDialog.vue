<script setup lang="ts">
import { CircleAlert, Download, FileWarning } from "@lucide/vue";
import { computed, onBeforeUnmount, ref, watch } from "vue";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { commands, toAppError, unwrap, type AppError, type ObjectPreview } from "@/services/tauri";
import { formatBytes } from "@/utils/format";

const props = defineProps<{ connectionId: string; bucket: string; objectKey: string }>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ download: [] }>();

const preview = ref<ObjectPreview | null>(null);
const objectUrl = ref<string | null>(null);
const text = ref("");
const loading = ref(false);
const error = ref<AppError | null>(null);
let requestSeq = 0;

const fileName = computed(() => props.objectKey.split("/").pop() ?? props.objectKey);
/** WebKitGTK has no built-in PDF viewer. */
const pdfUnsupported =
  navigator.userAgent.includes("Linux") && !navigator.userAgent.includes("Android");

function bytesFromBase64(data: string): Uint8Array<ArrayBuffer> {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function formatText(raw: string, contentType: string, key: string, truncated: boolean) {
  const isJson = contentType.includes("json") || key.toLowerCase().endsWith(".json");
  if (!isJson || truncated) return raw;
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

/**
 * Blob type from the preview kind, so bucket metadata can never turn a
 * preview into an HTML document. The backend already pins these types.
 */
function blobType(result: ObjectPreview): string {
  if (result.kind === "PDF") return "application/pdf";
  return result.contentType.startsWith("image/") ? result.contentType : "application/octet-stream";
}

function release() {
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value);
  objectUrl.value = null;
}

async function load() {
  const seq = ++requestSeq;
  release();
  preview.value = null;
  text.value = "";
  error.value = null;
  loading.value = true;
  try {
    const result = await unwrap(
      commands.previewObject(props.connectionId, props.bucket, props.objectKey),
    );
    if (seq !== requestSeq) return;
    const bytes = bytesFromBase64(result.dataBase64);
    if (result.kind === "TEXT") {
      const raw = new TextDecoder("utf-8", { fatal: false }).decode(bytes);
      text.value = formatText(raw, result.contentType, props.objectKey, result.truncated);
    } else {
      objectUrl.value = URL.createObjectURL(new Blob([bytes], { type: blobType(result) }));
    }
    preview.value = result;
  } catch (err) {
    if (seq === requestSeq) error.value = toAppError(err);
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}

watch(
  () => [open.value, props.objectKey] as const,
  ([isOpen]) => {
    if (isOpen) void load();
    else {
      requestSeq++;
      release();
    }
  },
);

onBeforeUnmount(release);
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="flex h-[85vh] max-w-5xl flex-col gap-0 p-0">
      <header class="flex items-center gap-3 border-b py-3 pr-12 pl-5">
        <div class="min-w-0 flex-1">
          <DialogTitle class="truncate text-[15px] font-semibold">{{ fileName }}</DialogTitle>
          <DialogDescription class="text-muted-foreground truncate text-xs">
            <template v-if="preview">
              <span class="font-mono">{{ preview.contentType }}</span>
              <span class="mx-1.5 opacity-40">·</span>
              {{ formatBytes(preview.size) }}
            </template>
            <template v-else>{{ objectKey }}</template>
          </DialogDescription>
        </div>
        <Button variant="outline" size="sm" @click="emit('download')">
          <Download /> {{ $t("transfers.downloadAction") }}
        </Button>
      </header>

      <div class="bg-muted/40 relative min-h-0 flex-1 overflow-hidden">
        <div v-if="loading" class="absolute inset-0 grid place-items-center">
          <div class="skeleton h-2/3 w-2/3 rounded-xl" :aria-label="$t('explorer.loading')" />
        </div>

        <div
          v-else-if="error"
          class="absolute inset-0 grid place-items-center p-8 text-center"
          role="alert"
        >
          <div class="max-w-sm">
            <CircleAlert class="text-muted-foreground mx-auto size-8" />
            <p class="mt-3 text-[13px]">{{ error.message }}</p>
            <Button class="mt-4" size="sm" @click="emit('download')">
              <Download /> {{ $t("transfers.downloadAction") }}
            </Button>
          </div>
        </div>

        <template v-else-if="preview">
          <div
            v-if="preview.kind === 'IMAGE' && objectUrl"
            class="preview-checker absolute inset-0 grid place-items-center overflow-auto p-6"
          >
            <img
              :src="objectUrl"
              :alt="fileName"
              class="max-h-full max-w-full object-contain shadow-[var(--shadow-float)]"
              draggable="false"
            />
          </div>

          <template v-else-if="preview.kind === 'PDF'">
            <div
              v-if="pdfUnsupported"
              class="absolute inset-0 grid place-items-center p-8 text-center"
            >
              <div class="max-w-sm">
                <FileWarning class="text-muted-foreground mx-auto size-8" />
                <p class="mt-3 text-[13px]">{{ $t("preview.pdfUnsupported") }}</p>
              </div>
            </div>
            <iframe
              v-else-if="objectUrl"
              :src="objectUrl"
              :title="fileName"
              class="absolute inset-0 size-full bg-white"
            />
          </template>

          <div v-else class="absolute inset-0 flex flex-col">
            <p
              v-if="preview.truncated"
              class="bg-warning/10 text-warning border-b px-5 py-1.5 text-xs"
            >
              {{ $t("preview.truncated") }}
            </p>
            <pre
              class="bg-background min-h-0 flex-1 overflow-auto p-5 font-mono text-xs leading-relaxed whitespace-pre select-text"
              >{{ text }}</pre>
          </div>
        </template>
      </div>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
/* Checkerboard so transparent images remain visible on both themes. */
.preview-checker {
  background-image:
    linear-gradient(45deg, var(--muted) 25%, transparent 25%),
    linear-gradient(-45deg, var(--muted) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, var(--muted) 75%),
    linear-gradient(-45deg, transparent 75%, var(--muted) 75%);
  background-size: 20px 20px;
  background-position:
    0 0,
    0 10px,
    10px -10px,
    -10px 0;
}
</style>
