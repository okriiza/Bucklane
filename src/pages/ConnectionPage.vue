<script setup lang="ts">
import { CircleAlert, CircleCheck, Pencil, PlugZap, Trash2 } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";

import BucketList from "@/components/buckets/BucketList.vue";
import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import ProviderMark from "@/components/connections/ProviderMark.vue";
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
    await router.push({ name: "home" });
  } catch (err) {
    toast.error(toAppError(err).message);
  }
}
</script>

<template>
  <div v-if="connection" class="mx-auto max-w-4xl px-10 pt-6 pb-12">
    <header class="flex items-start gap-4">
      <ProviderMark :provider="connection.provider" size="lg" />
      <div class="min-w-0 flex-1">
        <h1 class="truncate text-2xl font-semibold tracking-[-0.015em]">{{ connection.name }}</h1>
        <p class="text-muted-foreground mt-0.5 truncate text-[13px]">
          {{ profile?.label ?? connection.provider }}
          <span class="mx-1.5 opacity-50">·</span>
          <span class="font-mono text-xs select-text">
            {{ connection.endpoint ?? $t("connection.defaultEndpoint") }}
          </span>
        </p>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <Button variant="outline" :disabled="testing" @click="runTest">
          <PlugZap /> {{ testing ? $t("form.testing") : $t("connection.test") }}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          :title="$t('common.edit')"
          :aria-label="$t('common.edit')"
          @click="store.openEditForm(connection.id)"
        >
          <Pencil />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="hover:text-destructive"
          :title="$t('common.delete')"
          :aria-label="$t('common.delete')"
          @click="confirmDelete = true"
        >
          <Trash2 />
        </Button>
      </div>
    </header>

    <div
      v-if="testResult"
      class="bg-success/10 text-success mt-5 inline-flex items-center gap-2 rounded-md px-3 py-1.5 text-[13px] font-medium"
      role="status"
    >
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
      class="border-destructive/30 bg-destructive/8 text-destructive mt-5 grid gap-1 rounded-lg border p-3 text-[13px]"
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

    <dl class="mt-6 flex flex-wrap gap-x-8 gap-y-3 border-y py-4 text-[13px]">
      <div>
        <dt class="text-muted-foreground text-[11px]">{{ $t("connection.region") }}</dt>
        <dd class="mt-0.5 font-mono text-xs select-text">{{ connection.region }}</dd>
      </div>
      <div class="min-w-0">
        <dt class="text-muted-foreground text-[11px]">{{ $t("connection.accessKeyId") }}</dt>
        <dd class="mt-0.5 truncate font-mono text-xs select-text">{{ connection.accessKeyId }}</dd>
      </div>
      <div v-if="connection.forcePathStyle || connection.allowInsecureHttp">
        <dt class="text-muted-foreground text-[11px]">{{ $t("connection.options") }}</dt>
        <dd class="mt-1 flex gap-1.5">
          <span
            v-if="connection.forcePathStyle"
            class="bg-secondary text-secondary-foreground rounded px-1.5 py-0.5 text-[11px]"
          >
            {{ $t("connection.pathStyle") }}
          </span>
          <span
            v-if="connection.allowInsecureHttp"
            class="bg-warning/15 text-warning rounded px-1.5 py-0.5 text-[11px] font-medium"
          >
            {{ $t("connection.insecureHttp") }}
          </span>
        </dd>
      </div>
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
  <p v-else-if="!store.loading" class="text-muted-foreground p-10 text-sm">
    {{ $t("connection.notFound") }}
  </p>
</template>
