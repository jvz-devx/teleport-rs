/** Error from the transport layer (network, timeout, serialization). */
export type TransportError =
  | { type: "NetworkError"; message: string }
  | { type: "SerializationError"; message: string }
  | { type: "Timeout"; message: string }
  | { type: "ServerError"; status: number; body: string };

/** Application-level error returned by a Rust procedure. */
export type AppError<T = never> =
  | { type: "Unauthorized" }
  | { type: "Forbidden" }
  | { type: "NotFound" }
  | { type: "BadRequest"; message: string }
  | { type: "Internal"; message: string }
  | { type: "RateLimited" }
  | { type: "Detail"; detail: T };

/**
 * Result of an RPC call. Discriminated union with three branches:
 * - Success: `{ ok: true, data: T }`
 * - Application error (from Rust): `{ ok: false, error: AppError<E> }`
 * - Transport error (network/protocol): `{ ok: false, transport: TransportError }`
 */
export type RpcResult<T, E = never> =
  | { ok: true; data: T }
  | { ok: false; error: AppError<E> }
  | { ok: false; transport: TransportError };

export type HttpMethod = "GET" | "POST";
