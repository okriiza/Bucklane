import type { AppError } from "./bindings";

export * from "./bindings";

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

/** Error thrown by `unwrap` carrying the backend's structured `AppError`. */
export class CommandError extends Error {
  constructor(readonly appError: AppError) {
    super(appError.message);
    this.name = "CommandError";
  }
}

/** Converts a tauri-specta result into a value, throwing `CommandError` on failure. */
export async function unwrap<T>(result: Promise<CommandResult<T>>): Promise<T> {
  const r = await result;
  if (r.status === "error") throw new CommandError(r.error);
  return r.data;
}

/** Normalizes anything thrown by a command call into an `AppError` for display. */
export function toAppError(err: unknown): AppError {
  if (err instanceof CommandError) return err.appError;
  return {
    code: "INTERNAL_ERROR",
    message: "Could not reach the Bucklane backend.",
    details: err instanceof Error ? err.message : null,
    retryable: false,
  };
}
