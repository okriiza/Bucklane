import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  commands,
  type ListObjectsResponse,
  type ObjectMetadata,
  type StorageObject,
} from "@/services/tauri";

import { AUTO_LOAD_LIMIT, useExplorerStore } from "./explorer";

vi.mock("@/services/tauri/bindings", () => ({
  commands: {
    listObjects: vi.fn(),
    headObject: vi.fn(),
    moveObject: vi.fn(),
    copyObject: vi.fn(),
    deleteObjects: vi.fn(),
    executePrefixDelete: vi.fn(),
    preparePrefixRelocate: vi.fn(),
    executePrefixRelocate: vi.fn(),
  },
}));

const mocked = vi.mocked(commands);

function entry(key: string, isPrefix = false): StorageObject {
  return {
    key,
    name: key.split("/").filter(Boolean).at(-1)! + (isPrefix ? "/" : ""),
    size: isPrefix ? null : 1,
    lastModified: null,
    etag: null,
    storageClass: null,
    isPrefix,
  };
}

function page(objects: StorageObject[], next: string | null = null) {
  const data: ListObjectsResponse = {
    objects,
    nextContinuationToken: next,
    isTruncated: next !== null,
  };
  return { status: "ok" as const, data };
}

const docs = { connectionId: "c1", bucket: "b", prefix: "docs/" };

describe("useExplorerStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.resetAllMocks();
  });

  it("loads the first page and passes the prefix through", async () => {
    mocked.listObjects.mockResolvedValue(page([entry("docs/a.txt")]));
    const store = useExplorerStore();

    await store.open(docs);

    expect(mocked.listObjects).toHaveBeenCalledWith(
      expect.objectContaining({ prefix: "docs/", continuationToken: null, delimiter: "/" }),
    );
    expect(store.entries.map((e) => e.key)).toEqual(["docs/a.txt"]);
    expect(store.isTruncated).toBe(false);
  });

  it("loads the rest of a folder in the background, folders first", async () => {
    mocked.listObjects
      .mockResolvedValueOnce(page([entry("docs/a.txt")], "opaque==token"))
      .mockResolvedValueOnce(page([entry("docs/z/", true), entry("docs/b.txt")]));
    const store = useExplorerStore();

    await store.open(docs);
    await vi.waitFor(() => expect(store.autoLoading).toBe(false));

    expect(mocked.listObjects).toHaveBeenLastCalledWith(
      expect.objectContaining({ continuationToken: "opaque==token", pageSize: 1000 }),
    );
    expect(store.sortedEntries.map((e) => e.key)).toEqual(["docs/z/", "docs/a.txt", "docs/b.txt"]);
    expect(store.isTruncated).toBe(false);
  });

  it("stops loading ahead at the limit and leaves the rest to Load more", async () => {
    const big = (n: number) =>
      Array.from({ length: n }, (_, i) => entry(`docs/f${String(i).padStart(5, "0")}`));
    mocked.listObjects.mockImplementation(async () => page(big(1000), "more"));
    const store = useExplorerStore();

    await store.open(docs);
    await vi.waitFor(() => expect(store.autoLoading).toBe(false));

    expect(store.entries.length).toBe(AUTO_LOAD_LIMIT);
    expect(store.isTruncated).toBe(true);
    expect(store.loadingMore).toBe(false);
  });

  it("stops loading ahead when another folder opens", async () => {
    let release!: (v: ReturnType<typeof page>) => void;
    mocked.listObjects
      .mockResolvedValueOnce(page([entry("docs/a.txt")], "t1"))
      .mockReturnValueOnce(new Promise((r) => (release = r)))
      .mockResolvedValueOnce(page([entry("photos/cat.jpg")]));
    const store = useExplorerStore();

    await store.open(docs);
    await store.open({ ...docs, prefix: "photos/" });
    release(page([entry("docs/stale.txt")]));
    await vi.waitFor(() => expect(store.loading).toBe(false));

    expect(store.entries.map((e) => e.key)).toEqual(["photos/cat.jpg"]);
    expect(store.autoLoading).toBe(false);
  });

  it("keeps what loaded when a background page fails", async () => {
    mocked.listObjects
      .mockResolvedValueOnce(page([entry("docs/a.txt")], "t1"))
      .mockRejectedValueOnce(new Error("network"));
    const store = useExplorerStore();

    await store.open(docs);
    await vi.waitFor(() => expect(store.autoLoading).toBe(false));

    expect(store.entries.map((e) => e.key)).toEqual(["docs/a.txt"]);
    expect(store.error).toBeNull();
    expect(store.isTruncated).toBe(true);
  });

  it("searches by name prefix on the server and keeps it across refresh", async () => {
    mocked.listObjects.mockResolvedValue(page([entry("docs/report.pdf")]));
    const store = useExplorerStore();
    await store.open(docs);

    await store.searchServer("rep");
    await store.refresh();

    expect(mocked.listObjects).toHaveBeenLastCalledWith(
      expect.objectContaining({ prefix: "docs/rep", continuationToken: null }),
    );
    expect(store.search).toBe("rep");

    await store.clearSearch();
    expect(mocked.listObjects).toHaveBeenLastCalledWith(
      expect.objectContaining({ prefix: "docs/" }),
    );
  });

  it("drops the search when opening another folder", async () => {
    mocked.listObjects.mockResolvedValue(page([]));
    const store = useExplorerStore();
    await store.open(docs);
    await store.searchServer("rep");

    await store.open({ ...docs, prefix: "photos/" });

    expect(store.search).toBe("");
    expect(mocked.listObjects).toHaveBeenLastCalledWith(
      expect.objectContaining({ prefix: "photos/" }),
    );
  });

  it("prepares a folder move against the open bucket", async () => {
    mocked.listObjects.mockResolvedValue(page([]));
    mocked.preparePrefixRelocate.mockResolvedValue({
      status: "ok",
      data: {
        planId: "p1",
        mode: "MOVE",
        sourcePrefix: "docs/a/",
        destinationPrefix: "docs/b/",
        objectCount: 2,
        totalBytes: 2,
        conflictCount: 0,
        conflictSamples: [],
        expiresAt: "",
      },
    });
    const store = useExplorerStore();
    await store.open(docs);

    const plan = await store.prepareFolderRelocate("docs/a/", "docs/b/", "MOVE");

    expect(plan.planId).toBe("p1");
    expect(mocked.preparePrefixRelocate).toHaveBeenCalledWith({
      connectionId: "c1",
      bucket: "b",
      sourcePrefix: "docs/a/",
      destinationPrefix: "docs/b/",
      mode: "MOVE",
    });
  });

  it("refreshes the folder after a folder move, even if it fails", async () => {
    mocked.listObjects.mockResolvedValue(page([]));
    mocked.executePrefixRelocate.mockResolvedValue({
      status: "error",
      error: { code: "TRANSFER_FAILED", message: "boom", details: null, retryable: true },
    });
    const store = useExplorerStore();
    await store.open(docs);
    const before = mocked.listObjects.mock.calls.length;

    await expect(store.executeFolderRelocate("p1", "SKIP")).rejects.toThrow("boom");
    expect(mocked.listObjects.mock.calls.length).toBeGreaterThan(before);
  });

  it("remembers the chosen sort order", () => {
    const store = useExplorerStore();
    store.setSort({ key: "size", direction: "desc" });

    setActivePinia(createPinia());
    expect(useExplorerStore().sort).toEqual({ key: "size", direction: "desc" });
    localStorage.clear();
  });

  it("ignores a slow response for a folder the user already left", async () => {
    let resolveSlow!: (v: ReturnType<typeof page>) => void;
    mocked.listObjects
      .mockReturnValueOnce(new Promise((r) => (resolveSlow = r)))
      .mockResolvedValueOnce(page([entry("photos/cat.jpg")]));
    const store = useExplorerStore();

    const slow = store.open(docs);
    await store.open({ ...docs, prefix: "photos/" });
    resolveSlow(page([entry("docs/stale.txt")]));
    await slow;

    expect(store.location?.prefix).toBe("photos/");
    expect(store.entries.map((e) => e.key)).toEqual(["photos/cat.jpg"]);
    expect(store.loading).toBe(false);
  });

  it("reports listing errors", async () => {
    mocked.listObjects.mockResolvedValue({
      status: "error",
      error: { code: "ACCESS_DENIED", message: "Access denied.", details: null, retryable: false },
    });
    const store = useExplorerStore();

    await store.open(docs);

    expect(store.error?.code).toBe("ACCESS_DENIED");
    expect(store.entries).toEqual([]);
  });

  const metadata = (overrides: Partial<ObjectMetadata> = {}): ObjectMetadata => ({
    key: "docs/a.txt",
    size: 1,
    lastModified: null,
    contentType: "text/plain",
    cacheControl: null,
    contentDisposition: null,
    contentEncoding: null,
    contentLanguage: null,
    etag: null,
    storageClass: null,
    metadata: {},
    ...overrides,
  });

  it("shows saved metadata without reloading the folder", async () => {
    mocked.listObjects.mockResolvedValue(page([entry("docs/a.txt")]));
    mocked.headObject.mockResolvedValue({ status: "ok", data: metadata() });
    const store = useExplorerStore();
    await store.open(docs);
    await store.select("docs/a.txt");
    mocked.listObjects.mockClear();

    store.applyMetadata(
      metadata({ cacheControl: "no-cache", lastModified: "2026-11-01T00:00:00Z" }),
    );

    expect(store.metadata?.cacheControl).toBe("no-cache");
    expect(store.entries[0]?.lastModified).toBe("2026-11-01T00:00:00Z");
    expect(mocked.listObjects).not.toHaveBeenCalled();
  });

  it("loads metadata for the selected object", async () => {
    mocked.listObjects.mockResolvedValue(page([entry("docs/a.txt")]));
    mocked.headObject.mockResolvedValue({ status: "ok", data: metadata() });
    const store = useExplorerStore();
    await store.open(docs);

    await store.select("docs/a.txt");

    expect(mocked.headObject).toHaveBeenCalledWith("c1", "b", "docs/a.txt");
    expect(store.selected?.key).toBe("docs/a.txt");
    expect(store.metadata?.contentType).toBe("text/plain");
  });

  it("does not refresh when a move reports a conflict", async () => {
    mocked.listObjects.mockResolvedValue(page([entry("docs/a.txt")]));
    mocked.moveObject.mockResolvedValue({
      status: "ok",
      data: { kind: "conflict", existingSize: 3, existingLastModified: null },
    });
    const store = useExplorerStore();
    await store.open(docs);
    mocked.listObjects.mockClear();

    const outcome = await store.relocate("move", "docs/a.txt", "docs/b.txt", "ASK");

    expect(outcome.kind).toBe("conflict");
    expect(mocked.moveObject).toHaveBeenCalledWith(
      expect.objectContaining({ sourceKey: "docs/a.txt", conflictPolicy: "ASK" }),
    );
    expect(mocked.listObjects).not.toHaveBeenCalled();
  });

  it("merges file and folder delete results, keeping failures", async () => {
    mocked.listObjects.mockResolvedValue(page([]));
    mocked.deleteObjects.mockResolvedValue({
      status: "ok",
      data: {
        deleted: ["docs/a.txt"],
        failed: [
          {
            key: "docs/b.txt",
            error: { code: "ACCESS_DENIED", message: "Denied", details: null, retryable: false },
          },
        ],
      },
    });
    mocked.executePrefixDelete.mockResolvedValue({
      status: "ok",
      data: { deleted: ["docs/sub/", "docs/sub/c.txt"], failed: [] },
    });
    const store = useExplorerStore();
    await store.open(docs);

    const result = await store.deleteEntries(["docs/a.txt", "docs/b.txt"], ["plan-1"]);

    expect(mocked.executePrefixDelete).toHaveBeenCalledWith("plan-1");
    expect(result.deleted).toHaveLength(3);
    expect(result.failed.map((f) => f.key)).toEqual(["docs/b.txt"]);
  });

  it("clears checked items when navigating", async () => {
    mocked.listObjects.mockResolvedValue(page([entry("docs/a.txt")]));
    const store = useExplorerStore();
    await store.open(docs);
    store.toggleChecked("docs/a.txt", true);
    expect(store.checkedEntries).toHaveLength(1);

    await store.open({ ...docs, prefix: "other/" });

    expect(store.checkedKeys.size).toBe(0);
  });
});
