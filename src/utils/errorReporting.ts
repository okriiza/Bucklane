import { commands } from "@/services/tauri";

/** Caps how many errors one session writes to the log file. */
const MAX_REPORTS = 25;
let reported = 0;

function describe(err: unknown): string {
  if (err instanceof Error) {
    // The first stack frames are enough to locate the problem.
    const stack = err.stack?.split("\n").slice(1, 4).join(" | ") ?? "";
    return `${err.name}: ${err.message}${stack ? ` @ ${stack}` : ""}`;
  }
  return String(err);
}

/**
 * Logs an unexpected UI error to the console and Bucklane's log file, where
 * it can be attached to a bug report. The backend redacts URLs.
 */
export function reportError(err: unknown, context: string) {
  console.error(`[${context}]`, err);
  if (reported >= MAX_REPORTS) return;
  reported++;
  commands.logFrontendError(`[${context}] ${describe(err)}`).catch(() => {
    // The backend may be unavailable; the console still has the error.
  });
}

/** Installs global handlers for errors no component caught. */
export function installErrorReporting(app: { config: { errorHandler?: unknown } }) {
  app.config.errorHandler = (err: unknown, _instance: unknown, info: string) =>
    reportError(err, `vue: ${info}`);
  window.addEventListener("unhandledrejection", (event) =>
    reportError(event.reason, "unhandled promise"),
  );
  window.addEventListener("error", (event) => reportError(event.error ?? event.message, "window"));
}
