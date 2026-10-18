<script setup lang="ts">
import { CircleAlert } from "@lucide/vue";
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";

import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import { Button } from "@/components/ui/button";
import Label from "@/components/ui/label/Label.vue";
import NativeSelect from "@/components/ui/native-select/NativeSelect.vue";
import { toAppError, type AppError, type AppSettings, type Theme } from "@/services/tauri";
import { useSettingsStore } from "@/stores/settings";

const store = useSettingsStore();
const { t } = useI18n();

const form = reactive<AppSettings>({ ...store.settings });
const saving = ref(false);
const error = ref<AppError | null>(null);
const confirmReset = ref(false);

// Keep the form in sync when settings load after the page opened.
watch(
  () => store.settings,
  (saved) => Object.assign(form, saved),
);

const numberFields = [
  { key: "maxConcurrentTransfers", min: 1, max: 10, section: "transfers" },
  { key: "multipartThresholdMb", min: 5, max: 5120, section: "transfers" },
  { key: "multipartPartSizeMb", min: 5, max: 5120, section: "transfers" },
  { key: "explorerPageSize", min: 50, max: 1000, section: "explorer" },
] as const;

const dirty = computed(() => numberFields.some(({ key }) => form[key] !== store.settings[key]));

async function save() {
  saving.value = true;
  error.value = null;
  try {
    await store.save({ ...store.settings, ...pickNumbers() });
    toast.success(t("settings.saved"));
  } catch (err) {
    error.value = toAppError(err);
  } finally {
    saving.value = false;
  }
}

function pickNumbers() {
  return Object.fromEntries(numberFields.map(({ key }) => [key, form[key]])) as Pick<
    AppSettings,
    (typeof numberFields)[number]["key"]
  >;
}

async function reset() {
  try {
    await store.reset();
    Object.assign(form, store.settings);
    toast.success(t("settings.resetDone"));
  } catch (err) {
    error.value = toAppError(err);
  }
}

async function setCheckForUpdates(value: boolean) {
  form.checkForUpdates = value;
  try {
    await store.save({ ...store.settings, checkForUpdates: value });
  } catch (err) {
    error.value = toAppError(err);
    form.checkForUpdates = store.settings.checkForUpdates;
  }
}

/** The theme is applied and saved as soon as it is picked. */
async function setTheme(value: Theme) {
  form.theme = value;
  try {
    await store.save({ ...store.settings, theme: value });
  } catch (err) {
    error.value = toAppError(err);
    form.theme = store.settings.theme;
  }
}
</script>

<template>
  <div class="mx-auto max-w-2xl px-10 pt-6 pb-12">
    <h1 class="text-2xl font-semibold tracking-[-0.015em]">{{ $t("settings.title") }}</h1>

    <form class="mt-8 grid gap-10" @submit.prevent="save">
      <section class="grid gap-4">
        <h2 class="text-[15px] font-semibold tracking-tight">
          {{ $t("settings.appearance") }}
        </h2>
        <div class="grid max-w-xs gap-2">
          <Label for="s-theme">{{ $t("settings.theme") }}</Label>
          <NativeSelect
            id="s-theme"
            :model-value="form.theme"
            @update:model-value="(v) => setTheme(v as Theme)"
          >
            <option value="system">{{ $t("settings.themes.system") }}</option>
            <option value="light">{{ $t("settings.themes.light") }}</option>
            <option value="dark">{{ $t("settings.themes.dark") }}</option>
          </NativeSelect>
        </div>
      </section>

      <section
        v-for="section in ['transfers', 'explorer'] as const"
        :key="section"
        class="grid gap-4"
      >
        <h2 class="text-[15px] font-semibold tracking-tight">
          {{ $t(`settings.${section}`) }}
        </h2>
        <div
          v-for="field in numberFields.filter((f) => f.section === section)"
          :key="field.key"
          class="grid grid-cols-[1fr_8rem] items-center gap-x-6 gap-y-1"
        >
          <Label :for="`s-${field.key}`">{{ $t(`settings.fields.${field.key}.label`) }}</Label>
          <input
            :id="`s-${field.key}`"
            v-model.number="form[field.key]"
            type="number"
            :min="field.min"
            :max="field.max"
            required
            class="border-input bg-background focus-visible:border-ring focus-visible:ring-ring/25 dark:bg-input/30 tabular h-8 rounded-md border px-2.5 text-right font-mono text-[13px] shadow-[var(--shadow-soft)] outline-none focus-visible:ring-[3px]"
          />
          <p class="text-muted-foreground col-span-2 text-xs">
            {{ $t(`settings.fields.${field.key}.hint`, { min: field.min, max: field.max }) }}
          </p>
        </div>
      </section>

      <section class="grid gap-4">
        <h2 class="text-[15px] font-semibold tracking-tight">{{ $t("settings.updates") }}</h2>
        <div class="grid grid-cols-[auto_1fr] items-start gap-x-3 gap-y-1">
          <input
            id="s-check-updates"
            type="checkbox"
            class="accent-primary mt-0.5 size-4"
            :checked="form.checkForUpdates"
            @change="setCheckForUpdates(($event.target as HTMLInputElement).checked)"
          />
          <Label for="s-check-updates">{{ $t("settings.checkForUpdates") }}</Label>
          <p class="text-muted-foreground col-start-2 text-xs">
            {{ $t("settings.checkForUpdatesHint") }}
          </p>
        </div>
      </section>

      <p
        v-if="error"
        class="border-destructive/40 bg-destructive/10 text-destructive flex gap-2 rounded-md border p-3 text-sm"
        role="alert"
      >
        <CircleAlert class="mt-0.5 size-4 shrink-0" />
        {{ error.message }}
      </p>

      <div class="flex justify-between gap-2">
        <Button variant="ghost" @click="confirmReset = true">{{ $t("settings.reset") }}</Button>
        <Button type="submit" :disabled="saving || !dirty">
          {{ saving ? $t("form.saving") : $t("common.save") }}
        </Button>
      </div>
    </form>

    <ConfirmDialog
      v-model:open="confirmReset"
      :title="$t('settings.resetTitle')"
      :description="$t('settings.resetDescription')"
      :confirm-label="$t('settings.reset')"
      @confirm="reset"
    />
  </div>
</template>
