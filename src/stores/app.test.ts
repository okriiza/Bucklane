import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { commands } from "@/services/tauri";

import { useAppStore } from "./app";

vi.mock("@/services/tauri", () => ({
  commands: { health: vi.fn() },
}));

const health = vi.mocked(commands.health);

describe("useAppStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    health.mockReset();
  });

  it("is ready when the backend and database are healthy", async () => {
    health.mockResolvedValue({
      status: "ok",
      data: { appVersion: "0.1.0", databaseOk: true, databaseError: null },
    });
    const store = useAppStore();

    await store.checkHealth();

    expect(store.status).toBe("ready");
    expect(store.health?.appVersion).toBe("0.1.0");
  });

  it("reports an unavailable database", async () => {
    health.mockResolvedValue({
      status: "ok",
      data: {
        appVersion: "0.1.0",
        databaseOk: false,
        databaseError: {
          code: "DATABASE_UNAVAILABLE",
          message: "Bucklane could not open its local database.",
          details: null,
          retryable: false,
        },
      },
    });
    const store = useAppStore();

    await store.checkHealth();

    expect(store.status).toBe("database-unavailable");
  });

  it("is unreachable when IPC fails", async () => {
    health.mockRejectedValue(new Error("no Tauri runtime"));
    const store = useAppStore();

    await store.checkHealth();

    expect(store.status).toBe("unreachable");
  });
});
