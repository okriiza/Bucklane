import { onScopeDispose, watchEffect } from "vue";

import { useSettingsStore } from "@/stores/settings";

/** Applies the `appearance.theme` setting, following the OS when "system". */
export function useTheme() {
  const store = useSettingsStore();
  const query = window.matchMedia("(prefers-color-scheme: dark)");

  function apply() {
    const theme = store.settings.theme;
    const dark = theme === "dark" || (theme === "system" && query.matches);
    document.documentElement.classList.toggle("dark", dark);
  }

  watchEffect(apply);
  query.addEventListener("change", apply);
  onScopeDispose(() => query.removeEventListener("change", apply));
}
