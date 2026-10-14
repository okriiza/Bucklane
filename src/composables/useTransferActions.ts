import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";

import {
  commands,
  toAppError,
  unwrap,
  type EnqueueItemResult,
  type LocalFileSelection,
  type TransferConflictPolicy,
} from "@/services/tauri";
import { useConflictPromptStore } from "@/stores/conflictPrompt";

export interface UploadTarget {
  connectionId: string;
  bucket: string;
  prefix: string;
}

/**
 * Upload and download entry points shared by toolbar buttons, row actions,
 * and OS drag-and-drop. Destinations that already exist are never replaced
 * without asking (ADR-013): conflicting items are re-sent with the policy
 * the user picks.
 */
export function useTransferActions() {
  const { t } = useI18n();
  const conflicts = useConflictPromptStore();

  function report(results: EnqueueItemResult[]) {
    const queued = results.filter((r) => r.kind === "queued").length;
    const skipped = results.filter((r) => r.kind === "skipped");
    const rejected = results.filter((r) => r.kind === "rejected");
    if (queued) toast.success(t("transfers.queued", { count: queued }, queued));
    if (skipped.length) {
      toast.info(t("transfers.skipped", { count: skipped.length }, skipped.length), {
        description: skipped.map((s) => s.name).join(", "),
      });
    }
    for (const r of rejected) {
      toast.error(r.name, { description: r.error.message });
    }
  }

  /** Runs `enqueue` with ASK, then once more for conflicts with the user's choice. */
  async function withConflicts(
    direction: "UPLOAD" | "DOWNLOAD",
    enqueue: (
      items: string[] | null,
      policy: TransferConflictPolicy,
    ) => Promise<EnqueueItemResult[]>,
  ) {
    try {
      const first = await enqueue(null, "ASK");
      const conflicting = first.filter((r) => r.kind === "conflict");
      report(first);
      if (!conflicting.length) return;

      const choice = await conflicts.ask(
        conflicting.map((c) => c.name),
        direction,
      );
      if (!choice) return;
      report(
        await enqueue(
          conflicting.map((c) => c.item),
          choice,
        ),
      );
    } catch (err) {
      toast.error(toAppError(err).message);
    }
  }

  function uploadSelections(target: UploadTarget, files: LocalFileSelection[]) {
    if (!files.length) return Promise.resolve();
    return withConflicts("UPLOAD", (items, policy) =>
      unwrap(
        commands.enqueueUpload({
          ...target,
          selectionIds: items ?? files.map((f) => f.selectionId),
          conflictPolicy: policy,
        }),
      ),
    );
  }

  async function pickAndUpload(target: UploadTarget) {
    try {
      const files = await unwrap(commands.pickUploadFiles());
      await uploadSelections(target, files);
    } catch (err) {
      toast.error(toAppError(err).message);
    }
  }

  async function pickAndDownload(connectionId: string, bucket: string, keys: string[]) {
    if (!keys.length) return;
    try {
      const directory = await unwrap(commands.pickDownloadDirectory());
      if (!directory) return;
      await withConflicts("DOWNLOAD", (items, policy) =>
        unwrap(
          commands.enqueueDownload({
            connectionId,
            bucket,
            keys: items ?? keys,
            directorySelectionId: directory.selectionId,
            conflictPolicy: policy,
          }),
        ),
      );
    } catch (err) {
      toast.error(toAppError(err).message);
    }
  }

  return { uploadSelections, pickAndUpload, pickAndDownload };
}
