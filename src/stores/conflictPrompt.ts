import { defineStore } from "pinia";
import { ref } from "vue";

import type { TransferConflictPolicy, TransferDirection } from "@/services/tauri";

export type ConflictChoice = Exclude<TransferConflictPolicy, "ASK">;

/**
 * Asks the user how to resolve existing destinations for a batch of
 * transfers. One shared dialog (rendered in the layout) answers through a
 * promise so any caller can `await` the decision.
 */
export const useConflictPromptStore = defineStore("conflictPrompt", () => {
  const open = ref(false);
  const names = ref<string[]>([]);
  const direction = ref<TransferDirection>("UPLOAD");
  let resolver: ((choice: ConflictChoice | null) => void) | null = null;

  function ask(conflicting: string[], dir: TransferDirection) {
    resolver?.(null);
    names.value = conflicting;
    direction.value = dir;
    open.value = true;
    return new Promise<ConflictChoice | null>((resolve) => (resolver = resolve));
  }

  function answer(choice: ConflictChoice | null) {
    open.value = false;
    resolver?.(choice);
    resolver = null;
  }

  return { open, names, direction, ask, answer };
});
