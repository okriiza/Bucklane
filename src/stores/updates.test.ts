import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { commands, events } from "@/services/tauri";

import { useUpdateStore } from "./updates";

vi.mock("@/services/tauri/bindings", () => ({
  commands: { checkForUpdate: vi.fn(), installUpdate: vi.fn(), restartApp: vi.fn() },
  events: { updateProgress: { listen: vi.fn() } },
}));

const mocked = vi.mocked(commands);
const update = { version: "0.4.0", currentVersion: "0.3.0", notes: null };

describe("useUpdateStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.resetAllMocks();
  });

  it("reports an available update", async () => {
    mocked.checkForUpdate.mockResolvedValue({ status: "ok", data: update });
    const store = useUpdateStore();

    expect(await store.check()).toEqual(update);
    expect(store.status).toBe("available");
  });

  it("reports when already up to date", async () => {
    mocked.checkForUpdate.mockResolvedValue({ status: "ok", data: null });
    const store = useUpdateStore();

    expect(await store.check()).toBeNull();
    expect(store.status).toBe("upToDate");
  });

  it("records check errors instead of throwing", async () => {
    mocked.checkForUpdate.mockResolvedValue({
      status: "error",
      error: { code: "ENDPOINT_UNREACHABLE", message: "offline", details: null, retryable: true },
    });
    const store = useUpdateStore();

    await store.check();
    expect(store.status).toBe("error");
    expect(store.error?.message).toBe("offline");
  });

  it("tracks download progress while installing", async () => {
    mocked.checkForUpdate.mockResolvedValue({ status: "ok", data: update });
    const unlisten = vi.fn();
    vi.mocked(events.updateProgress.listen).mockImplementation(async (handler) => {
      handler({ event: "update-progress", id: 1, payload: { downloaded: 512, total: 1024 } });
      return unlisten;
    });
    mocked.installUpdate.mockResolvedValue({ status: "ok", data: null });
    const store = useUpdateStore();
    await store.check();

    await store.install();

    expect(store.downloaded).toBe(512);
    expect(store.total).toBe(1024);
    expect(store.status).toBe("ready");
    expect(unlisten).toHaveBeenCalled();
  });
});
