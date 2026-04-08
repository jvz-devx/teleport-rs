import type { AppError, RpcResult, TransportError } from "./types";

/** Type guard: is this a transport-level error? */
export function isTransportError<T, E>(
  result: RpcResult<T, E>,
): result is { ok: false; transport: TransportError } {
  return !result.ok && "transport" in result;
}

/** Type guard: is this an application-level error from Rust? */
export function isAppError<T, E>(
  result: RpcResult<T, E>,
): result is { ok: false; error: AppError<E> } {
  return !result.ok && "error" in result;
}

/**
 * Unwrap a successful result or throw.
 *
 * Useful when you've already handled errors and want to extract the data,
 * or in contexts where throwing is acceptable (e.g. SvelteKit load functions).
 */
export function unwrap<T, E>(result: RpcResult<T, E>): T {
  if (result.ok) return result.data;
  if ("transport" in result) {
    const t = result.transport;
    const msg = "message" in t ? t.message : `Server error ${t.status}`;
    throw new Error(msg);
  }
  throw new Error(result.error.type);
}
