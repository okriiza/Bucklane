import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { commands, type ListObjectsResponse, type StorageObject } from "@/services/tauri";

import { useExplorerStore } from "./explorer";

vi.mock("@/services/tauri/bindings", () => ({
  commands: { listObjects: vi.fn(), headObject: vi.fn() },
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
    mocked.listObjects.mockResolvedValue(page([entry("docs/a.txt")], "t1"));
    const store = useExplorerStore();

    await store.open(docs);

    expect(mocked.listObjects).toHaveBeenCalledWith(
      expect.objectContaining({ prefix: "docs/", continuationToken: null, delimiter: "/" }),
    );
    expect(store.entries.map((e) => e.key)).toEqual(["docs/a.txt"]);
    expect(store.isTruncated).toBe(true);
  });

  it("appends pages with the opaque token and keeps folders first", async () => {
    mocked.listObjects
      .mockResolvedValueOnce(page([entry("docs/a.txt")], "opaque==token"))
      .mockResolvedValueOnce(page([entry("docs/z/", true), entry("docs/b.txt")]));
    const store = useExplorerStore();

    await store.open(docs);
    await store.loadMore();

    expect(mocked.listObjects).toHaveBeenLastCalledWith(
      expect.objectContaining({ continuationToken: "opaque==token" }),
    );
    expect(store.entries.map((e) => e.key)).toEqual(["docs/z/", "docs/a.txt", "docs/b.txt"]);
    expect(store.isTruncated).toBe(false);
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

  it("loads metadata for the selected object", async () => {
    mocked.listObjects.mockResolvedValue(page([entry("docs/a.txt")]));
    mocked.headObject.mockResolvedValue({
      status: "ok",
      data: {
        key: "docs/a.txt",
        size: 1,
        lastModified: null,
        contentType: "text/plain",
        etag: null,
        storageClass: null,
        metadata: {},
      },
    });
    const store = useExplorerStore();
    await store.open(docs);

    await store.select("docs/a.txt");

    expect(mocked.headObject).toHaveBeenCalledWith("c1", "b", "docs/a.txt");
    expect(store.selected?.key).toBe("docs/a.txt");
    expect(store.metadata?.contentType).toBe("text/plain");
  });
});
