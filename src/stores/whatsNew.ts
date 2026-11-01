import { defineStore } from "pinia";
import { ref } from "vue";

import { releaseNotes } from "@/utils/changelog";

const SEEN_KEY = "bucklane.lastSeenVersion";

/** The "What's new" dialog: shown once after an update, and from About. */
export const useWhatsNewStore = defineStore("whatsNew", () => {
  const open = ref(false);
  const version = ref<string | null>(null);

  function show(v: string) {
    version.value = v;
    open.value = true;
  }

  /**
   * Shows the notes of `current` if the app was updated since the last
   * launch. A fresh install only records the version. Versions before
   * 0.9.0 recorded nothing, so a missing record with `returningUser` (saved
   * connections exist) also counts as an update.
   */
  function checkAfterUpdate(current: string, returningUser = false) {
    let seen: string | null = null;
    try {
      seen = localStorage.getItem(SEEN_KEY);
      localStorage.setItem(SEEN_KEY, current);
    } catch {
      // Storage unavailable: skip the dialog rather than show it every time.
      return;
    }
    const updated = seen ? seen !== current : returningUser;
    if (updated && releaseNotes(current)) show(current);
  }

  return { open, version, show, checkAfterUpdate };
});
