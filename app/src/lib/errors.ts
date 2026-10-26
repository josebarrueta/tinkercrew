/** Native IPC rejects with plain objects, not necessarily JavaScript Errors. */
export function errorMessage(
  cause: unknown,
  fallback = "Something went wrong. Please retry.",
): string {
  const message =
    typeof cause === "string"
      ? cause
      : cause &&
          typeof cause === "object" &&
          "message" in cause &&
          typeof cause.message === "string"
        ? cause.message
        : fallback;
  return message.trim().slice(0, 600) || fallback;
}
