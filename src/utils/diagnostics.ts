import type { AppSettings, DiagnosticsInfo } from "@/services/tauri";

export interface DiagnosticsContext {
  info: DiagnosticsInfo;
  settings: AppSettings;
  /** Provider types of saved connections; names and endpoints are left out. */
  providers: string[];
  transfersByStatus: Record<string, number>;
}

function countBy(values: string[]): string {
  const counts = new Map<string, number>();
  for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
  return [...counts].map(([k, n]) => `${k} ×${n}`).join(", ") || "none";
}

/**
 * Plain-text summary for bug reports. It deliberately contains no connection
 * names, endpoints, buckets, object keys, or credentials.
 */
export function formatDiagnostics({
  info,
  settings,
  providers,
  transfersByStatus,
}: DiagnosticsContext): string {
  const transfers =
    Object.entries(transfersByStatus)
      .filter(([, n]) => n > 0)
      .map(([status, n]) => `${status.toLowerCase()} ${n}`)
      .join(", ") || "none";
  return [
    `Bucklane ${info.appVersion}`,
    `OS: ${info.os} (${info.arch})`,
    `Tauri: ${info.tauriVersion}`,
    `WebView: ${info.webviewVersion ?? "unknown"}`,
    `Database: ${info.databaseOk ? "ok" : "unavailable"}`,
    `Connections: ${countBy(providers)}`,
    `Transfers: ${transfers}`,
    `Settings: concurrency ${settings.maxConcurrentTransfers}, multipart ≥${settings.multipartThresholdMb} MB` +
      ` in ${settings.multipartPartSizeMb} MB parts, page size ${settings.explorerPageSize},` +
      ` update check ${settings.checkForUpdates ? "on" : "off"}`,
  ].join("\n");
}
