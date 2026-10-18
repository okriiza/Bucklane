import type { StorageObject } from "@/services/tauri";

export type SortKey = "name" | "size" | "lastModified";
export type SortDirection = "asc" | "desc";
export interface SortState {
  key: SortKey;
  direction: SortDirection;
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

function compareFiles(a: StorageObject, b: StorageObject, key: SortKey): number {
  if (key === "size") return (a.size ?? 0) - (b.size ?? 0);
  if (key === "lastModified") {
    return Date.parse(a.lastModified ?? "") - Date.parse(b.lastModified ?? "") || 0;
  }
  return collator.compare(a.name, b.name);
}

/**
 * Sorts loaded entries, always keeping folders above files. Folders have no
 * size or date, so they stay in name order for those columns. Ties fall back
 * to the name so the order is stable across page loads.
 */
export function sortEntries(entries: StorageObject[], sort: SortState): StorageObject[] {
  const sign = sort.direction === "asc" ? 1 : -1;
  const byName = (a: StorageObject, b: StorageObject) => collator.compare(a.name, b.name);
  const folders = entries.filter((e) => e.isPrefix);
  const files = entries.filter((e) => !e.isPrefix);

  folders.sort((a, b) => (sort.key === "name" ? sign : 1) * byName(a, b));
  files.sort((a, b) => sign * (compareFiles(a, b, sort.key) || byName(a, b)));
  return [...folders, ...files];
}

/** Clicking the active column flips it; a new column starts in its natural order. */
export function nextSort(current: SortState, key: SortKey): SortState {
  if (current.key === key) {
    return { key, direction: current.direction === "asc" ? "desc" : "asc" };
  }
  // Newest first and largest first are the useful starting points.
  return { key, direction: key === "name" ? "asc" : "desc" };
}
