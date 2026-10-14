<script setup lang="ts">
import { CircleAlert, CircleCheck, Pencil, Trash2 } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";

import BucketList from "@/components/buckets/BucketList.vue";
import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import { Button } from "@/components/ui/button";
import { toAppError, type AppError, type TestConnectionResult } from "@/services/tauri";
import { useConnectionStore } from "@/stores/connections";

const props = defineProps<{ id: string }>();

const store = useConnectionStore();
const router = useRouter();
const { t } = useI18n();

const connection = computed(() => store.byId(props.id));
const profile = computed(() => connection.value && store.profileFor(connection.value.provider));

const testing = ref(false);
const testResult = ref<TestConnectionResult | null>(null);
const testError = ref<AppError | null>(null);
const confirmDelete = ref(false);

watch(
  () => props.id,
  () => {
    testResult.value = null;
    testError.value = null;
  },
);

async function runTest() {
  testing.value = true;
  testResult.value = null;
  testError.value = null;
  try {
    testResult.value = await store.test({ connectionId: props.id, config: null, testBucket: null });
  } catch (err) {
    testError.value = toAppError(err);
  } finally {
    testing.value = false;
  }
}

async function remove() {
  try {
    const result = await store.remove(props.id);
    if (result.secretRemoved) toast.success(t("connection.deleted"));
    else toast.warning(t("connection.secretNotRemoved"));
    await router.push({ name: "welcome" });
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <div v-if="connection" class="p-8">
    <header class="flex items-start justify-between gap-4">
      <div class="min-w-0">
        <h1 class="truncate text-xl font-semibold tracking-tight">{{ connection.name }}</h1>
        <p class="text-muted-foreground text-sm">{{ profile?.label ?? connection.provider }}</p>
      </div>
      <div class="flex shrink-0 gap-2">
        <Button variant="outline" size="sm" :disabled="testing" @click="runTest">
          {{ testing ? $t("form.testing") : $t("connection.test") }}
        </Button>
        <Button variant="outline" size="sm" @click="store.openEditForm(connection.id)">
          <Pencil /> {{ $t("common.edit") }}
        </Button>
        <Button variant="outline" size="sm" @click="confirmDelete = true">
          <Trash2 /> {{ $t("common.delete") }}
        </Button>
      </div>
    </header>

    <div v-if="testResult" class="text-success mt-4 flex items-center gap-2 text-sm" role="status">
      <CircleCheck class="size-4" />
      <span v-if="testResult.method === 'LIST_BUCKETS'">
        {{
          $t("form.testOkList", { count: testResult.bucketCount ?? 0 }, testResult.bucketCount ?? 0)
        }}
      </span>
      <span v-else>{{ $t("form.testOkHead") }}</span>
    </div>
    <div
      v-if="testError"
      class="border-destructive/40 bg-destructive/10 text-destructive mt-4 grid gap-1 rounded-md border p-3 text-sm"
      role="alert"
    >
      <div class="flex gap-2">
        <CircleAlert class="mt-0.5 size-4 shrink-0" />
        <span>{{ testError.message }}</span>
      </div>
      <code v-if="testError.details" class="pl-6 font-mono text-xs opacity-80 select-text">
        {{ testError.details }}
      </code>
    </div>

    <dl class="mt-6 grid max-w-xl grid-cols-[10rem_1fr] gap-x-4 gap-y-2 text-sm">
      <dt class="text-muted-foreground">{{ $t("connection.endpoint") }}</dt>
      <dd class="truncate font-mono text-xs leading-5 select-text">
        {{ connection.endpoint ?? $t("connection.defaultEndpoint") }}
      </dd>
      <dt class="text-muted-foreground">{{ $t("connection.region") }}</dt>
      <dd class="font-mono text-xs leading-5 select-text">{{ connection.region }}</dd>
      <dt class="text-muted-foreground">{{ $t("connection.accessKeyId") }}</dt>
      <dd class="font-mono text-xs leading-5 select-text">{{ connection.accessKeyId }}</dd>
      <template v-if="connection.forcePathStyle">
        <dt class="text-muted-foreground">{{ $t("connection.pathStyle") }}</dt>
        <dd>✓</dd>
      </template>
      <template v-if="connection.allowInsecureHttp">
        <dt class="text-muted-foreground">{{ $t("connection.insecureHttp") }}</dt>
        <dd class="text-amber-600 dark:text-amber-400">✓</dd>
      </template>
    </dl>

    <BucketList :connection-id="connection.id" />

    <ConfirmDialog
      v-model:open="confirmDelete"
      :title="$t('connection.deleteTitle', { name: connection.name })"
      :description="$t('connection.deleteDescription')"
      :confirm-label="$t('common.delete')"
      destructive
      @confirm="remove"
    />
  </div>
  <p v-else-if="!store.loading" class="text-muted-foreground p-8 text-sm">
    {{ $t("connection.notFound") }}
  </p>
</template>
