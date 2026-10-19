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
import { useSettingsStore } from "@/stores/settings";
import { sortEntries, type SortState } from "@/utils/sort";

export interface ExplorerLocation {
  connectionId: string;
  bucket: string;
  prefix: string;
}

const SORT_STORAGE_KEY = "bucklane.explorer.sort";
/**
 * After the first page, the rest of a folder loads in the background up to
 * this many entries. S3 returns folders and files interleaved by name, so a
 * subfolder can sit pages deep; loading ahead puts every folder on top and
 * lets sort and filter cover the whole folder. Beyond it, "Load more" remains.
 */
export const AUTO_LOAD_LIMIT = 10_000;
/** ListObjectsV2 returns at most 1000 keys per request. */
const BACKGROUND_PAGE_SIZE = 1000;
const DEFAULT_SORT: SortState = { key: "name", direction: "asc" };

function loadSort(): SortState {
  try {
    const saved = JSON.parse(localStorage.getItem(SORT_STORAGE_KEY) ?? "null") as SortState | null;
    if (
      saved &&
      ["name", "size", "lastModified"].includes(saved.key) &&
      ["asc", "desc"].includes(saved.direction)
    ) {
      return saved;
    }
  } catch {
    // Storage may be unavailable; the default order is fine.
  }
  return DEFAULT_SORT;
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
  /** Remaining pages are loading in the background. */
  const autoLoading = ref(false);
  const error = ref<AppError | null>(null);
  /**
   * Server-side name search: lists `prefix + search` instead of the folder.
   * S3 can only match the start of a key, so this finds names that begin
   * with the query (case-sensitive) in the current folder.
   */
  const search = ref("");
  const sort = ref<SortState>(loadSort());

  const selectedKey = ref<string | null>(null);
  const metadata = ref<ObjectMetadata | null>(null);
  const metadataError = ref<AppError | null>(null);
  const metadataLoading = ref(false);

  /** Multi-selection for bulk actions (FIL-007); separate from the details selection. */
  const checkedKeys = ref<Set<string>>(new Set());

  let listSeq = 0;
  let metadataSeq = 0;

  const sortedEntries = computed(() => sortEntries(entries.value, sort.value));

  function setSort(next: SortState) {
    sort.value = next;
    try {
      localStorage.setItem(SORT_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Not persisted; the order still applies for this session.
    }
  }

  const checkedEntries = computed(() => entries.value.filter((e) => checkedKeys.value.has(e.key)));

  const selected = computed(
    () => entries.value.find((e) => e.key === selectedKey.value && !e.isPrefix) ?? null,
  );

  async function fetchPage(
    loc: ExplorerLocation,
    token: string | null,
    pageSize = useSettingsStore().settings.explorerPageSize,
  ) {
    return unwrap(
      commands.listObjects({
        connectionId: loc.connectionId,
        bucket: loc.bucket,
        prefix: loc.prefix + search.value || null,
        delimiter: "/",
        continuationToken: token,
        pageSize,
      }),
    );
  }

  /** Opens a folder; a new location always starts without a search. */
  function open(loc: ExplorerLocation) {
    search.value = "";
    return load(loc);
  }

  async function load(loc: ExplorerLocation) {
    const seq = ++listSeq;
    location.value = { ...loc };
    entries.value = [];
    nextToken.value = null;
    isTruncated.value = false;
    error.value = null;
    loading.value = true;
    loadingMore.value = false;
    autoLoading.value = false;
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
    if (seq === listSeq && isTruncated.value) void autoLoad(seq);
  }

  /**
   * Loads the remaining pages of the current listing until it is complete,
   * reaches `AUTO_LOAD_LIMIT`, fails, or the user opens something else. A
   * failure is left for "Load more" to retry rather than shown as an error.
   */
  async function autoLoad(seq: number) {
    const loc = location.value;
    if (!loc) return;
    autoLoading.value = true;
    loadingMore.value = true;
    try {
      while (
        seq === listSeq &&
        isTruncated.value &&
        nextToken.value &&
        entries.value.length < AUTO_LOAD_LIMIT
      ) {
        const page = await fetchPage(loc, nextToken.value, BACKGROUND_PAGE_SIZE);
        if (seq !== listSeq) return;
        entries.value = [...entries.value, ...page.objects];
        nextToken.value = page.nextContinuationToken;
        isTruncated.value = page.isTruncated;
      }
    } catch {
      // Keep what loaded; the "Load more" button offers a retry.
    } finally {
      if (seq === listSeq) {
        autoLoading.value = false;
        loadingMore.value = false;
      }
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
      entries.value = [...entries.value, ...page.objects];
      nextToken.value = page.nextContinuationToken;
      isTruncated.value = page.isTruncated;
    } catch (err) {
      if (seq === listSeq) error.value = toAppError(err);
    } finally {
      if (seq === listSeq) loadingMore.value = false;
    }
  }

  /** Keeps the active search, if any. */
  function refresh() {
    if (location.value) return load(location.value);
  }

  function searchServer(query: string) {
    if (!location.value || query.includes("/")) return;
    search.value = query;
    return load(location.value);
  }

  function clearSearch() {
    if (!search.value) return;
    search.value = "";
    return refresh();
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
    sortedEntries,
    sort,
    setSort,
    search,
    searchServer,
    clearSearch,
    isTruncated,
    loading,
    loadingMore,
    autoLoading,
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
