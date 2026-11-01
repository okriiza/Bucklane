import { ArrowDown, ArrowUp, CopyPlus, FolderInput } from "@lucide/vue";

import type { TransferDto } from "@/services/tauri";

export type TransferKind = "UPLOAD" | "DOWNLOAD" | "COPY" | "MOVE";

/** A move is a COPY transfer that deletes its source afterwards. */
export function transferKind(t: TransferDto): TransferKind {
  return t.direction === "COPY" && t.source?.deleteAfterCopy ? "MOVE" : t.direction;
}

export const TRANSFER_ICONS = {
  UPLOAD: ArrowUp,
  DOWNLOAD: ArrowDown,
  COPY: CopyPlus,
  MOVE: FolderInput,
} as const;

/** The file name; folder markers (`photos/2026/`) show as `2026/`. */
export function transferName(t: TransferDto): string {
  if (t.direction === "UPLOAD") return t.localFileName;
  const key = t.objectKey;
  if (key.endsWith("/")) return `${key.slice(0, -1).split("/").pop() ?? key}/`;
  return key.split("/").pop() ?? key;
}
