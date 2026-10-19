import { describe, expect, it } from "vitest";

import { DEFAULT_SETTINGS } from "@/stores/settings";

import { formatDiagnostics } from "./diagnostics";

describe("formatDiagnostics", () => {
  it("summarizes the environment without connection details", () => {
    const text = formatDiagnostics({
      info: {
        appVersion: "0.5.0",
        tauriVersion: "2.12.0",
        webviewVersion: null,
        os: "macos",
        arch: "aarch64",
        databaseOk: true,
      },
      settings: DEFAULT_SETTINGS,
      providers: ["AWS_S3", "SUPABASE", "AWS_S3"],
      transfersByStatus: { RUNNING: 1, FAILED: 2, QUEUED: 0 },
    });

    expect(text).toContain("Bucklane 0.5.0");
    expect(text).toContain("OS: macos (aarch64)");
    expect(text).toContain("WebView: unknown");
    expect(text).toContain("Connections: AWS_S3 ×2, SUPABASE ×1");
    expect(text).toContain("Transfers: running 1, failed 2");
    expect(text).not.toContain("queued");
  });
});
