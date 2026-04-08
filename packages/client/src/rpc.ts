import qs from "qs";
import { getConfig } from "./config";
import type { AppError, HttpMethod, RpcResult, TransportError } from "./types";

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
  const config = getConfig();
  const controller = new AbortController();
  const timeoutId = setTimeout(
    () => controller.abort(),
    config.timeout ?? 30_000,
  );

  try {
    const headers: Record<string, string> = {
      ...(config.headers ? await config.headers() : {}),
    };

    const init: RequestInit = {
      method,
      headers,
      credentials: config.credentials,
      signal: controller.signal,
    };

    let url = `${config.baseUrl}${path}`;

    if (method === "GET" && input) {
      const queryString = qs.stringify(input, { skipNulls: true });
      if (queryString) url = `${url}?${queryString}`;
    } else if (method === "POST" && input !== undefined) {
      headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(input);
    }

    const response = await fetch(url, init);

    clearTimeout(timeoutId);

    if (!response.ok) {
      try {
        const errorBody = await response.json();
        const result = { kind: "error" as const, ok: false as const, error: errorBody as AppError<E> };
        config.onError?.({ type: "app", error: errorBody as AppError<unknown> });
        return result;
      } catch {
        const transportError: TransportError = {
          type: "ServerError",
          status: response.status,
          body: await response.text(),
        };
        config.onError?.({ type: "transport", error: transportError });
        return { kind: "transport", ok: false, transport: transportError };
      }
    }

    const text = await response.text();
    const data = text ? (JSON.parse(text) as T) : (undefined as T);
    return { kind: "success", ok: true, data };
  } catch (err) {
    clearTimeout(timeoutId);

    if (err instanceof DOMException && err.name === "AbortError") {
      const transportError: TransportError = {
        type: "Timeout",
        message: `Request timed out after ${config.timeout ?? 30_000}ms`,
      };
      config.onError?.({ type: "transport", error: transportError });
      return { kind: "transport", ok: false, transport: transportError };
    }

    const transportError: TransportError = {
      type: "NetworkError",
      message: err instanceof Error ? err.message : String(err),
    };
    config.onError?.({ type: "transport", error: transportError });
    return { kind: "transport", ok: false, transport: transportError };
  }
}
