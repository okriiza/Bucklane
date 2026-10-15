import { defineStore } from "pinia";
import { computed, ref } from "vue";

import { commands, type AppError, type HealthStatus } from "@/services/tauri";

export type BackendStatus = "checking" | "ready" | "database-unavailable" | "unreachable";

export const useAppStore = defineStore("app", () => {
  const health = ref<HealthStatus | null>(null);
  const error = ref<AppError | null>(null);
  const checking = ref(false);

  const status = computed<BackendStatus>(() => {
    if (checking.value && !health.value) return "checking";
    if (!health.value) return "unreachable";
    return health.value.databaseOk ? "ready" : "database-unavailable";
  });

  async function checkHealth() {
    checking.value = true;
    try {
      const result = await commands.health();
      if (result.status === "ok") {
        health.value = result.data;
        error.value = null;
      } else {
        health.value = null;
        error.value = result.error;
      }
    } catch {
      // IPC itself failed (e.g. running in a plain browser without Tauri).
      health.value = null;
    } finally {
      checking.value = false;
    }
  }

  return { health, error, status, checkHealth };
});
