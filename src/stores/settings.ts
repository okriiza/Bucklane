import { defineStore } from "pinia";
import { ref } from "vue";

import { commands, toAppError, unwrap, type AppError, type AppSettings } from "@/services/tauri";

/** Mirrors backend defaults so the UI is usable before settings load. */
export const DEFAULT_SETTINGS: AppSettings = {
  theme: "system",
  maxConcurrentTransfers: 3,
  multipartThresholdMb: 64,
  multipartPartSizeMb: 16,
  explorerPageSize: 100,
  checkForUpdates: true,
};

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<AppSettings>({ ...DEFAULT_SETTINGS });
  const loadError = ref<AppError | null>(null);

  async function load() {
    try {
      settings.value = await unwrap(commands.getSettings());
      loadError.value = null;
    } catch (err) {
      // Keep defaults; the app stays usable without stored preferences.
      loadError.value = toAppError(err);
    }
  }

  async function save(next: AppSettings) {
    settings.value = await unwrap(commands.updateSettings(next));
  }

  async function reset() {
    settings.value = await unwrap(commands.resetSettings());
  }

  return { settings, loadError, load, save, reset };
});
