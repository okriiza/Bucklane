import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  commands,
  toAppError,
  unwrap,
  type AppError,
  type ConflictPolicy,
  type DeleteObjectsResult,
  type ObjectMetadata,
  type ObjectOperationOutcome,
  type PrefixDeletePlan,
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

  /** Multi-selection for bulk actions (FIL-007); separate from the details selection. */
  const checkedKeys = ref<Set<string>>(new Set());

  let listSeq = 0;
  let metadataSeq = 0;

  const checkedEntries = computed(() => entries.value.filter((e) => checkedKeys.value.has(e.key)));

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
    checkedKeys.value = new Set();

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

  function toggleChecked(key: string, checked: boolean) {
    const next = new Set(checkedKeys.value);
    if (checked) next.add(key);
    else next.delete(key);
    checkedKeys.value = next;
  }

  function setAllChecked(keys: string[], checked: boolean) {
    checkedKeys.value = checked ? new Set(keys) : new Set();
  }

  function requireLocation() {
    if (!location.value) throw new Error("No folder is open.");
    return location.value;
  }

  async function createFolder(name: string) {
    const loc = requireLocation();
    const key = await unwrap(
      commands.createFolder({
        connectionId: loc.connectionId,
        bucket: loc.bucket,
        prefix: loc.prefix,
        name,
      }),
    );
    await refresh();
    return key;
  }

  /** Copy or move one object; a `conflict` outcome changes nothing. */
  async function relocate(
    mode: "copy" | "move",
    sourceKey: string,
    destinationKey: string,
    conflictPolicy: ConflictPolicy,
    { refresh: refreshAfter = true } = {},
  ): Promise<ObjectOperationOutcome> {
    const loc = requireLocation();
    const input = {
      connectionId: loc.connectionId,
      bucket: loc.bucket,
      sourceKey,
      destinationKey,
      conflictPolicy,
    };
    const outcome = await unwrap(
      mode === "copy" ? commands.copyObject(input) : commands.moveObject(input),
    );
    if (refreshAfter && outcome.kind !== "conflict") await refresh();
    return outcome;
  }

  function prepareFolderDelete(prefix: string): Promise<PrefixDeletePlan> {
    const loc = requireLocation();
    return unwrap(commands.preparePrefixDelete(loc.connectionId, loc.bucket, prefix));
  }

  /** Deletes files directly and folders through their confirmed plans. */
  async function deleteEntries(
    fileKeys: string[],
    planIds: string[],
  ): Promise<DeleteObjectsResult> {
    const loc = requireLocation();
    const results: DeleteObjectsResult[] = [];
    if (fileKeys.length) {
      results.push(
        await unwrap(
          commands.deleteObjects({
            connectionId: loc.connectionId,
            bucket: loc.bucket,
            keys: fileKeys,
          }),
        ),
      );
    }
    for (const planId of planIds) {
      results.push(await unwrap(commands.executePrefixDelete(planId)));
    }
    await refresh();
    return {
      deleted: results.flatMap((r) => r.deleted),
      failed: results.flatMap((r) => r.failed),
    };
  }

  return {
    location,
    checkedKeys,
    checkedEntries,
    toggleChecked,
    setAllChecked,
    createFolder,
    relocate,
    prepareFolderDelete,
    deleteEntries,
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
