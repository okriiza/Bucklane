import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  commands,
  toAppError,
  unwrap,
  type AppError,
  type ObjectMetadata,
  type StorageObject,
} from "@/services/tauri";

export const PAGE_SIZE = 100;

export interface ExplorerLocation {
  connectionId: string;
  bucket: string;
  prefix: string;
}

/**
 * Listing state for one folder (TSD §10.2). Pages are fetched on demand and
 * never cached across folders. Each request carries a sequence number so a
 * slow response for a folder the user already left is discarded.
 */
export const useExplorerStore = defineStore("explorer", () => {
  const location = ref<ExplorerLocation | null>(null);
  const entries = ref<StorageObject[]>([]);
  const nextToken = ref<string | null>(null);
  const isTruncated = ref(false);
  const loading = ref(false);
  const loadingMore = ref(false);
  const error = ref<AppError | null>(null);

  const selectedKey = ref<string | null>(null);
  const metadata = ref<ObjectMetadata | null>(null);
  const metadataError = ref<AppError | null>(null);
  const metadataLoading = ref(false);

  let listSeq = 0;
  let metadataSeq = 0;

  const selected = computed(
    () => entries.value.find((e) => e.key === selectedKey.value && !e.isPrefix) ?? null,
  );

  async function fetchPage(loc: ExplorerLocation, token: string | null) {
    return unwrap(
      commands.listObjects({
        connectionId: loc.connectionId,
        bucket: loc.bucket,
        prefix: loc.prefix || null,
        delimiter: "/",
        continuationToken: token,
        pageSize: PAGE_SIZE,
      }),
    );
  }

  async function open(loc: ExplorerLocation) {
    const seq = ++listSeq;
    location.value = { ...loc };
    entries.value = [];
    nextToken.value = null;
    isTruncated.value = false;
    error.value = null;
    loading.value = true;
    clearSelection();

    try {
      const page = await fetchPage(loc, null);
      if (seq !== listSeq) return;
      entries.value = page.objects;
      nextToken.value = page.nextContinuationToken;
      isTruncated.value = page.isTruncated;
    } catch (err) {
      if (seq === listSeq) error.value = toAppError(err);
    } finally {
      if (seq === listSeq) loading.value = false;
    }
  }

  async function loadMore() {
    const loc = location.value;
    if (!loc || !isTruncated.value || !nextToken.value || loadingMore.value) return;
    const seq = listSeq;
    loadingMore.value = true;
    try {
      const page = await fetchPage(loc, nextToken.value);
      if (seq !== listSeq) return;
      // A page boundary can split folders from files; keep folders grouped.
      const merged = [...entries.value, ...page.objects];
      entries.value = [...merged.filter((e) => e.isPrefix), ...merged.filter((e) => !e.isPrefix)];
      nextToken.value = page.nextContinuationToken;
      isTruncated.value = page.isTruncated;
    } catch (err) {
      if (seq === listSeq) error.value = toAppError(err);
    } finally {
      loadingMore.value = false;
    }
  }

  function refresh() {
    if (location.value) return open(location.value);
  }

  async function select(key: string | null) {
    const seq = ++metadataSeq;
    selectedKey.value = key;
    metadata.value = null;
    metadataError.value = null;
    const loc = location.value;
    if (!key || !loc) return;

    metadataLoading.value = true;
    try {
      const result = await unwrap(commands.headObject(loc.connectionId, loc.bucket, key));
      if (seq === metadataSeq) metadata.value = result;
    } catch (err) {
      if (seq === metadataSeq) metadataError.value = toAppError(err);
    } finally {
      if (seq === metadataSeq) metadataLoading.value = false;
    }
  }

  function clearSelection() {
    metadataSeq++;
    selectedKey.value = null;
    metadata.value = null;
    metadataError.value = null;
    metadataLoading.value = false;
  }

  return {
    location,
    entries,
    isTruncated,
    loading,
    loadingMore,
    error,
    selectedKey,
    selected,
    metadata,
    metadataError,
    metadataLoading,
    open,
    loadMore,
    refresh,
    select,
    clearSelection,
  };
});
