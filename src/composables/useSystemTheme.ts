import { onScopeDispose } from "vue";

/**
 * Mirrors the OS color scheme onto the `dark` class.
 * Replaced by the `appearance.theme` setting once SettingsService lands.
 */
export function useSystemTheme() {
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const apply = () => document.documentElement.classList.toggle("dark", query.matches);

  apply();
  query.addEventListener("change", apply);
  onScopeDispose(() => query.removeEventListener("change", apply));
}
