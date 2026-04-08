import { TeleportError, TransportFailure } from "./errors";
import type { AppError, RpcResult, TransportError } from "./types";

/** Type guard: is this a transport-level error? */
export function isTransportError<T, E>(
  result: RpcResult<T, E>,
): result is { kind: "transport"; ok: false; transport: TransportError } {
  return result.kind === "transport";
}

/** Type guard: is this an application-level error from Rust? */
export function isAppError<T, E>(
  result: RpcResult<T, E>,
): result is { kind: "error"; ok: false; error: AppError<E> } {
  return result.kind === "error";
}

/**
 * Unwrap a successful result or throw.
 *
 * @deprecated Use `rpcUnwrap()` instead, which throws typed errors.
 */
export function unwrap<T, E>(result: RpcResult<T, E>): T {
  return rpcUnwrap(result);
}

/**
 * Unwrap a successful result or throw a typed error.
 *
 * - Transport errors throw `TransportFailure`
 * - Application errors throw `TeleportError` (preserving the full `AppError<E>`)
 *
 * Use in SvelteKit remote functions or any context where throwing is acceptable.
 */
export function rpcUnwrap<T, E>(result: RpcResult<T, E>): T {
  if (result.kind === "success") return result.data;
  if (result.kind === "transport") throw new TransportFailure(result.transport);
  throw new TeleportError(result.error);
}

/**
 * Extract data from a successful result, or transform the error.
 * Transport errors still throw `TransportFailure`.
 */
export function mapError<T, E, R>(
  result: RpcResult<T, E>,
  handler: (error: AppError<E>) => R,
): T | R {
  if (result.kind === "success") return result.data;
  if (result.kind === "transport") throw new TransportFailure(result.transport);
  return handler(result.error);
}
