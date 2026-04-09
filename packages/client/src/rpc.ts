import { createClient } from "./client";
import { getConfig } from "./config";
import type { HttpMethod, RpcResult } from "./types";

/**
 * Core RPC function. Generated client code delegates here.
 *
 * - GET requests encode input as query params via `qs` (supports nested objects/arrays).
 * - POST requests send input as JSON body.
 * - Returns a discriminated `RpcResult` — never throws.
 */
export async function rpc<T, E>(
  method: HttpMethod,
  path: string,
  input: unknown,
): Promise<RpcResult<T, E>> {
  const client = createClient(getConfig());
  return client.rpc(method, path, input);
}
