import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { commands, type TransferDto } from "@/services/tauri";

import { useTransferStore } from "./transfers";

vi.mock("@/services/tauri/bindings", () => ({
  commands: { listTransfers: vi.fn(), retryTransfer: vi.fn() },
  events: { transferUpdate: { listen: vi.fn() } },
}));

const mocked = vi.mocked(commands);

function transfer(overrides: Partial<TransferDto> = {}): TransferDto {
  return {
    id: "t1",
    connectionId: "c1",
    connectionNameSnapshot: "MinIO",
    direction: "UPLOAD",
    bucket: "b",
    objectKey: "docs/a.txt",
    localFileName: "a.txt",
    source: null,
    bytesTotal: 10,
    bytesTransferred: 0,
    status: "QUEUED",
    retryCount: 0,
    error: null,
    resumableBytes: null,
    paused: false,
    createdAt: "2026-10-15T00:00:00.000Z",
    startedAt: null,
    completedAt: null,
    ...overrides,
  };
}

describe("useTransferStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.resetAllMocks();
  });

  it("does not count paused downloads as failed", () => {
    const store = useTransferStore();
    store.upsert(transfer({ id: "a", direction: "DOWNLOAD", status: "FAILED", paused: true }));
    store.upsert(transfer({ id: "b", status: "FAILED" }));

    expect(store.failedCount).toBe(1);
    expect(store.pausedCount).toBe(1);
  });

  it("orders transfers newest first and counts active ones", async () => {
    mocked.listTransfers.mockResolvedValue({
      status: "ok",
      data: [
        transfer({ id: "old", createdAt: "2026-10-14T00:00:00.000Z", status: "COMPLETED" }),
        transfer({ id: "new", status: "RUNNING" }),
      ],
    });
    const store = useTransferStore();

    await store.load();

    expect(store.transfers.map((t) => t.id)).toEqual(["new", "old"]);
    expect(store.activeCount).toBe(1);
    expect(store.hasFinished).toBe(true);
  });

  it("notifies completion listeners exactly once per transfer", () => {
    const store = useTransferStore();
    const listener = vi.fn();
    store.onCompleted(listener);

    store.upsert(transfer({ status: "RUNNING", bytesTransferred: 5 }));
    store.upsert(transfer({ status: "COMPLETED", bytesTransferred: 10 }));
    store.upsert(transfer({ status: "COMPLETED", bytesTransferred: 10 }));

    expect(listener).toHaveBeenCalledTimes(1);
    expect(store.transfers[0]?.bytesTransferred).toBe(10);
  });

  it("replaces the row with the backend state after a retry", async () => {
    mocked.retryTransfer.mockResolvedValue({
      status: "ok",
      data: transfer({ status: "QUEUED", retryCount: 1 }),
    });
    const store = useTransferStore();
    store.upsert(transfer({ status: "FAILED" }));

    await store.retry("t1");

    expect(store.transfers[0]?.status).toBe("QUEUED");
    expect(store.transfers[0]?.retryCount).toBe(1);
  });

  it("measures a smoothed speed and estimates time left", () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const store = useTransferStore();
    const running = (bytes: number) =>
      transfer({ status: "RUNNING", bytesTotal: 10_000_000, bytesTransferred: bytes });

    store.upsert(running(0));
    expect(store.speedOf("t1")).toBeNull();

    vi.setSystemTime(1_000);
    store.upsert(running(1_000_000)); // 1 MB/s
    expect(store.speedOf("t1")).toBe(1_000_000);
    expect(store.etaOf(store.transfers[0]!)).toBe(9);

    vi.setSystemTime(2_000);
    store.upsert(running(3_000_000)); // 2 MB/s instant, smoothed toward it
    expect(store.speedOf("t1")).toBeCloseTo(1_300_000);

    store.upsert(transfer({ status: "COMPLETED", bytesTransferred: 10_000_000 }));
    expect(store.speedOf("t1")).toBeNull();
    vi.useRealTimers();
  });

  it("restarts measurement when progress goes backwards", () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const store = useTransferStore();
    store.upsert(transfer({ status: "RUNNING", bytesTransferred: 5_000_000 }));
    vi.setSystemTime(1_000);
    store.upsert(transfer({ status: "RUNNING", bytesTransferred: 0 }));
    expect(store.speedOf("t1")).toBeNull();
    vi.useRealTimers();
  });
});
