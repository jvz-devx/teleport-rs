import type { AppError, TransportError } from "./types";

type TeleportErrorMatch<E, K extends AppError<E>["type"]> =
  TeleportError<E> & {
    appError: Extract<AppError<E>, { type: K }>;
    detail: K extends "Detail" ? E : undefined;
  };

/**
 * Error thrown by rpcUnwrap() when the RPC call fails with an application error.
 * Carries the original typed error for downstream inspection.
 */
export class TeleportError<E = never> extends Error {
  readonly appError: AppError<E>;

  constructor(error: AppError<E>) {
    super(error.type);
    this.name = "TeleportError";
    this.appError = error;
  }

  /** Check if this is a specific error variant. */
  is<K extends AppError<E>["type"]>(type: K): this is TeleportErrorMatch<E, K> {
    return this.appError.type === type;
  }

  /** Get the detail payload (only present on Detail variant). */
  get detail(): E | undefined {
    return this.appError.type === "Detail" ? this.appError.detail : undefined;
  }
}

/**
 * Error thrown for transport-level failures (network, timeout, server error).
 */
export class TransportFailure extends Error {
  readonly transportError: TransportError;

  constructor(error: TransportError) {
    const msg =
      "message" in error ? error.message : `Server error ${error.status}`;
    super(msg);
    this.name = "TransportFailure";
    this.transportError = error;
  }
}
