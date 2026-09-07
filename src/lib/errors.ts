/**
 * IPC commands reject with a `String` per the contract, but a transport or
 * serialisation failure throws an `Error`. Both have to read the same way in
 * the UI, so normalisation lives here rather than being written twice and
 * drifting — which it already had.
 */
export function message(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "Unknown error";
}
