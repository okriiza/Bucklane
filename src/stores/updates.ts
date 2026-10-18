import { defineStore } from "pinia";
import { ref } from "vue";

import {
  commands,
  events,
  toAppError,
  unwrap,
  type AppError,
  type UpdateInfo,
} from "@/services/tauri";

export type UpdateStatus =
  "idle" | "checking" | "upToDate" | "available" | "installing" | "ready" | "error";

/**
 * In-app updates. Checking, downloading and signature verification happen in
 * Rust; this store only tracks what to show.
 */
export const useUpdateStore = defineStore("updates", () => {
  const status = ref<UpdateStatus>("idle");
  const update = ref<UpdateInfo | null>(null);
  const error = ref<AppError | null>(null);
  const downloaded = ref(0);
  const total = ref<number | null>(null);

  /** Returns the available update, or null. Errors are recorded, not thrown. */
  async function check(): Promise<UpdateInfo | null> {
    if (status.value === "checking" || status.value === "installing") return update.value;
    status.value = "checking";
    error.value = null;
    try {
      update.value = await unwrap(commands.checkForUpdate());
      status.value = update.value ? "available" : "upToDate";
    } catch (err) {
      error.value = toAppError(err);
      status.value = "error";
    }
    return update.value;
  }

  /** Downloads and installs; on Windows the app exits during this call. */
  async function install() {
    if (!update.value || status.value === "installing") return;
    status.value = "installing";
    error.value = null;
    downloaded.value = 0;
    total.value = null;
    const unlisten = await events.updateProgress.listen(({ payload }) => {
      downloaded.value = payload.downloaded ?? 0;
      total.value = payload.total;
    });
    try {
      await unwrap(commands.installUpdate());
      status.value = "ready";
    } catch (err) {
      error.value = toAppError(err);
      status.value = "error";
    } finally {
      unlisten();
    }
  }

  function restart() {
    return commands.restartApp();
  }

  function openReleasePage(version: string) {
    return unwrap(commands.openReleasePage(version));
  }

  return { status, update, error, downloaded, total, check, install, restart, openReleasePage };
});
