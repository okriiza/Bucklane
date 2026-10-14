import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { CommandError, commands, type ConnectionDto } from "@/services/tauri";

import { useConnectionStore } from "./connections";

vi.mock("@/services/tauri/bindings", () => ({
  commands: {
    listConnections: vi.fn(),
    listProviderProfiles: vi.fn(),
    createConnection: vi.fn(),
    deleteConnection: vi.fn(),
  },
}));

const mocked = vi.mocked(commands);

const connection: ConnectionDto = {
  id: "c1",
  name: "Local MinIO",
  provider: "minio",
  endpoint: "http://localhost:9000",
  region: "us-east-1",
  accessKeyId: "minioadmin",
  forcePathStyle: true,
  allowInsecureHttp: true,
  createdAt: "2026-10-14T00:00:00.000Z",
  updatedAt: "2026-10-14T00:00:00.000Z",
};

describe("useConnectionStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.resetAllMocks();
    mocked.listProviderProfiles.mockResolvedValue([]);
    mocked.listConnections.mockResolvedValue({ status: "ok", data: [connection] });
  });

  it("loads connections", async () => {
    const store = useConnectionStore();
    await store.load();

    expect(store.connections).toEqual([connection]);
    expect(store.byId("c1")?.name).toBe("Local MinIO");
    expect(store.loadError).toBeNull();
  });

  it("surfaces load errors without throwing", async () => {
    mocked.listConnections.mockResolvedValue({
      status: "error",
      error: {
        code: "DATABASE_UNAVAILABLE",
        message: "Bucklane could not open its local database.",
        details: null,
        retryable: false,
      },
    });
    const store = useConnectionStore();
    await store.load();

    expect(store.loadError?.code).toBe("DATABASE_UNAVAILABLE");
  });

  it("throws a CommandError with the backend error on failed create", async () => {
    mocked.createConnection.mockResolvedValue({
      status: "error",
      error: {
        code: "CONNECTION_INVALID",
        message: "Enter the secret access key.",
        details: null,
        retryable: false,
      },
    });
    const store = useConnectionStore();

    await expect(
      store.create({
        name: "x",
        provider: "aws",
        endpoint: null,
        region: "us-east-1",
        accessKeyId: "AKIA",
        secretAccessKey: null,
        forcePathStyle: false,
        allowInsecureHttp: false,
      }),
    ).rejects.toBeInstanceOf(CommandError);
  });

  it("reloads the list after deleting", async () => {
    mocked.deleteConnection.mockResolvedValue({ status: "ok", data: { secretRemoved: true } });
    const store = useConnectionStore();

    const result = await store.remove("c1");

    expect(result.secretRemoved).toBe(true);
    expect(mocked.listConnections).toHaveBeenCalled();
  });

  it("tracks the shared form target", () => {
    const store = useConnectionStore();
    store.openEditForm("c1");
    expect(store.formTarget).toBe("c1");
    store.closeForm();
    expect(store.formTarget).toBeNull();
  });
});
