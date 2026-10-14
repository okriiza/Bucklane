import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  commands,
  toAppError,
  unwrap,
  type AppError,
  type BucketBookmark,
  type BucketInfo,
} from "@/services/tauri";

export interface BucketEntry {
  name: string;
  /** Label shown in the UI: bookmark display name, or the bucket name. */
  label: string;
  region: string | null;
  createdAt: string | null;
  bookmark: BucketBookmark | null;
  /** `false` for bookmarks not returned by ListBuckets (e.g. no permission). */
  listed: boolean;
}

/** Buckets of the connection currently shown, merged with its bookmarks. */
export const useBucketStore = defineStore("buckets", () => {
  const connectionId = ref<string | null>(null);
  const buckets = ref<BucketInfo[]>([]);
  const bookmarks = ref<BucketBookmark[]>([]);
  const loading = ref(false);
  /** ListBuckets failure; bookmarks remain usable (BKT-001). */
  const listError = ref<AppError | null>(null);
  const bookmarkError = ref<AppError | null>(null);
  let requestSeq = 0;

  const entries = computed<BucketEntry[]>(() => {
    const byName = new Map<string, BucketEntry>();
    for (const b of buckets.value) {
      byName.set(b.name, {
        name: b.name,
        label: b.name,
        region: b.region,
        createdAt: b.createdAt,
        bookmark: null,
        listed: true,
      });
    }
    for (const bm of bookmarks.value) {
      const existing = byName.get(bm.bucketName);
      const label = bm.displayName ?? bm.bucketName;
      byName.set(
        bm.bucketName,
        existing
          ? { ...existing, label, bookmark: bm }
          : {
              name: bm.bucketName,
              label,
              region: null,
              createdAt: null,
              bookmark: bm,
              listed: false,
            },
      );
    }
    return [...byName.values()].sort((a, b) =>
      a.label.localeCompare(b.label, undefined, { sensitivity: "base" }),
    );
  });

  async function load(id: string) {
    const seq = ++requestSeq;
    if (connectionId.value !== id) {
      buckets.value = [];
      bookmarks.value = [];
    }
    connectionId.value = id;
    loading.value = true;

    const [listResult, bookmarkResult] = await Promise.allSettled([
      unwrap(commands.listBuckets(id)),
      unwrap(commands.listBucketBookmarks(id)),
    ]);
    if (seq !== requestSeq) return; // a newer load superseded this one

    if (listResult.status === "fulfilled") {
      buckets.value = listResult.value;
      listError.value = null;
    } else {
      buckets.value = [];
      listError.value = toAppError(listResult.reason);
    }
    if (bookmarkResult.status === "fulfilled") {
      bookmarks.value = bookmarkResult.value;
      bookmarkError.value = null;
    } else {
      bookmarkError.value = toAppError(bookmarkResult.reason);
    }
    loading.value = false;
  }

  async function addBookmark(bucketName: string, displayName: string | null = null) {
    const id = connectionId.value;
    if (!id) return;
    const bookmark = await unwrap(
      commands.addBucketBookmark({ connectionId: id, bucketName, displayName }),
    );
    if (!bookmarks.value.some((b) => b.id === bookmark.id)) {
      bookmarks.value = [...bookmarks.value, bookmark];
    }
    return bookmark;
  }

  async function removeBookmark(bookmarkId: string) {
    await unwrap(commands.removeBucketBookmark(bookmarkId));
    bookmarks.value = bookmarks.value.filter((b) => b.id !== bookmarkId);
  }

  return {
    connectionId,
    entries,
    loading,
    listError,
    bookmarkError,
    load,
    addBookmark,
    removeBookmark,
  };
});
