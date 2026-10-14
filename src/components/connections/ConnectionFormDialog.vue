<script setup lang="ts">
import { CircleCheck, CircleAlert, TriangleAlert } from "@lucide/vue";
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import Input from "@/components/ui/input/Input.vue";
import Label from "@/components/ui/label/Label.vue";
import NativeSelect from "@/components/ui/native-select/NativeSelect.vue";
import {
  toAppError,
  type AppError,
  type ConnectionConfigInput,
  type ProviderType,
  type TestConnectionResult,
} from "@/services/tauri";
import { useConnectionStore } from "@/stores/connections";

const store = useConnectionStore();
const router = useRouter();
const { t } = useI18n();

const open = computed({
  get: () => store.formTarget !== null,
  set: (value) => {
    if (!value) store.closeForm();
  },
});
const editingId = computed(() =>
  store.formTarget && store.formTarget !== "new" ? store.formTarget : null,
);

// Local, short-lived form state. The secret is cleared whenever the dialog
// closes so it never outlives the form.
const form = reactive({
  name: "",
  provider: "aws" as ProviderType,
  endpoint: "",
  region: "",
  accessKeyId: "",
  secretAccessKey: "",
  testBucket: "",
  forcePathStyle: false,
  allowInsecureHttp: false,
});

const busy = ref<"test" | "save" | null>(null);
const testResult = ref<TestConnectionResult | null>(null);
const error = ref<AppError | null>(null);

const profile = computed(() => store.profileFor(form.provider));
const isHttp = computed(() => form.endpoint.trim().toLowerCase().startsWith("http://"));

watch(open, (isOpen) => {
  if (isOpen) reset();
  else form.secretAccessKey = "";
});

// Apply provider defaults when the provider changes on a new connection.
watch(
  () => form.provider,
  (provider, previous) => {
    if (editingId.value) return;
    const next = store.profileFor(provider);
    const prev = store.profileFor(previous);
    if (!next) return;
    form.forcePathStyle = next.defaultForcePathStyle;
    if (!form.region || form.region === prev?.defaultRegion) {
      form.region = next.defaultRegion ?? "";
    }
  },
);

watch(
  () => ({ ...form }),
  () => {
    testResult.value = null;
  },
);

function reset() {
  const existing = editingId.value ? store.byId(editingId.value) : undefined;
  const provider = existing?.provider ?? "aws";
  const defaults = store.profileFor(provider);
  Object.assign(form, {
    name: existing?.name ?? "",
    provider,
    endpoint: existing?.endpoint ?? "",
    region: existing?.region ?? defaults?.defaultRegion ?? "",
    accessKeyId: existing?.accessKeyId ?? "",
    secretAccessKey: "",
    testBucket: "",
    forcePathStyle: existing?.forcePathStyle ?? defaults?.defaultForcePathStyle ?? false,
    allowInsecureHttp: existing?.allowInsecureHttp ?? false,
  });
  testResult.value = null;
  error.value = null;
}

function toInput(): ConnectionConfigInput {
  const blankToNull = (value: string) => (value.trim() ? value : null);
  return {
    name: form.name,
    provider: form.provider,
    endpoint: blankToNull(form.endpoint),
    region: form.region,
    accessKeyId: form.accessKeyId,
    secretAccessKey: blankToNull(form.secretAccessKey),
    forcePathStyle: form.forcePathStyle,
    allowInsecureHttp: form.allowInsecureHttp,
  };
}

async function runTest() {
  busy.value = "test";
  error.value = null;
  try {
    testResult.value = await store.test({
      connectionId: editingId.value,
      config: toInput(),
      testBucket: form.testBucket.trim() || null,
    });
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    busy.value = null;
  }
}

async function save() {
  busy.value = "save";
  error.value = null;
  try {
    if (editingId.value) {
      await store.update(editingId.value, toInput());
      toast.success(t("form.updated"));
    } else {
      const created = await store.create(toInput());
      toast.success(t("form.created"));
      await router.push({ name: "connection", params: { id: created.id } });
    }
    store.closeForm();
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    busy.value = null;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-xl">
      <div class="space-y-1">
        <DialogTitle class="text-lg font-semibold">
          {{ editingId ? $t("form.editTitle") : $t("form.createTitle") }}
        </DialogTitle>
        <DialogDescription class="text-muted-foreground text-sm">
          {{ $t("form.description") }}
        </DialogDescription>
      </div>

      <form class="grid gap-4" @submit.prevent="save">
        <div class="grid grid-cols-2 gap-4">
          <div class="grid gap-2">
            <Label for="cf-provider">{{ $t("form.provider") }}</Label>
            <NativeSelect id="cf-provider" v-model="form.provider">
              <option v-for="p in store.profiles" :key="p.provider" :value="p.provider">
                {{ p.label }}
              </option>
            </NativeSelect>
          </div>
          <div class="grid gap-2">
            <Label for="cf-name">{{ $t("form.name") }}</Label>
            <Input
              id="cf-name"
              v-model="form.name"
              :placeholder="$t('form.namePlaceholder')"
              autocomplete="off"
            />
          </div>
        </div>

        <div class="grid gap-2">
          <Label for="cf-endpoint">
            {{ $t("form.endpoint") }}
            <span v-if="!profile?.endpointRequired" class="text-muted-foreground font-normal">
              ({{ $t("common.optional") }})
            </span>
          </Label>
          <Input
            id="cf-endpoint"
            v-model="form.endpoint"
            :placeholder="profile?.endpointPlaceholder ?? ''"
            autocomplete="off"
            spellcheck="false"
          />
          <p v-if="!profile?.endpointRequired" class="text-muted-foreground text-xs">
            {{ $t("form.endpointOptional") }}
          </p>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="grid gap-2">
            <Label for="cf-region">{{ $t("form.region") }}</Label>
            <Input
              id="cf-region"
              v-model="form.region"
              :placeholder="profile?.regionPlaceholder ?? ''"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
          <div class="grid gap-2">
            <Label for="cf-bucket">
              {{ $t("form.testBucket") }}
              <span class="text-muted-foreground font-normal">({{ $t("common.optional") }})</span>
            </Label>
            <Input
              id="cf-bucket"
              v-model="form.testBucket"
              :title="$t('form.testBucketHint')"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="grid gap-2">
            <Label for="cf-access-key">{{ $t("form.accessKeyId") }}</Label>
            <Input
              id="cf-access-key"
              v-model="form.accessKeyId"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
          <div class="grid gap-2">
            <Label for="cf-secret">{{ $t("form.secretAccessKey") }}</Label>
            <Input
              id="cf-secret"
              v-model="form.secretAccessKey"
              type="password"
              autocomplete="new-password"
              :placeholder="editingId ? $t('form.secretKeepCurrent') : ''"
            />
          </div>
        </div>

        <div class="flex flex-wrap gap-x-6 gap-y-2 text-sm">
          <label class="flex items-center gap-2">
            <input v-model="form.forcePathStyle" type="checkbox" class="accent-primary size-4" />
            {{ $t("form.forcePathStyle") }}
          </label>
          <label class="flex items-center gap-2">
            <input v-model="form.allowInsecureHttp" type="checkbox" class="accent-primary size-4" />
            {{ $t("form.allowInsecureHttp") }}
          </label>
        </div>

        <p
          v-if="form.allowInsecureHttp || isHttp"
          class="flex gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-300"
          role="note"
        >
          <TriangleAlert class="mt-px size-4 shrink-0" />
          {{ $t("form.insecureWarning") }}
        </p>

        <div
          v-if="testResult"
          class="text-success flex items-center gap-2 text-sm"
          role="status"
          data-testid="test-success"
        >
          <CircleCheck class="size-4" />
          <span v-if="testResult.method === 'LIST_BUCKETS'">
            {{
              $t(
                "form.testOkList",
                { count: testResult.bucketCount ?? 0 },
                testResult.bucketCount ?? 0,
              )
            }}
          </span>
          <span v-else>{{ $t("form.testOkHead") }}</span>
        </div>

        <div
          v-if="error"
          class="border-destructive/40 bg-destructive/10 text-destructive grid gap-1 rounded-md border p-3 text-sm"
          role="alert"
        >
          <div class="flex gap-2">
            <CircleAlert class="mt-0.5 size-4 shrink-0" />
            <span>{{ error.message }}</span>
          </div>
          <code v-if="error.details" class="pl-6 font-mono text-xs opacity-80 select-text">
            {{ error.details }}
          </code>
        </div>

        <div class="flex items-center justify-between gap-2 pt-2">
          <Button variant="outline" :disabled="busy !== null" @click="runTest">
            {{ busy === "test" ? $t("form.testing") : $t("form.test") }}
          </Button>
          <div class="flex gap-2">
            <Button variant="ghost" :disabled="busy !== null" @click="store.closeForm()">
              {{ $t("common.cancel") }}
            </Button>
            <Button type="submit" :disabled="busy !== null">
              {{ busy === "save" ? $t("form.saving") : $t("common.save") }}
            </Button>
          </div>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
