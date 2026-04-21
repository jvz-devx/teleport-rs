import qs from "qs";
import type { AppError, HttpMethod, RpcResult, TransportError } from "./types";
import type { RpcConfig } from "./config";

export interface TeleportClient {
  rpc<T, E>(method: HttpMethod, path: string, input: unknown): Promise<RpcResult<T, E>>;
  readonly config: Readonly<RpcConfig>;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isAppErrorPayload(value: unknown): value is AppError<unknown> {
  if (!isRecord(value) || typeof value.type !== "string") {
    return false;
  }

  switch (value.type) {
    case "Unauthorized":
    case "Forbidden":
    case "NotFound":
    case "RateLimited":
      return true;
    case "BadRequest":
    case "Internal":
      return typeof value.message === "string";
    case "Detail":
      return "detail" in value;
    default:
      return false;
  }
}

function matchesAppErrorStatus(error: AppError<unknown>, status: number): boolean {
  switch (error.type) {
    case "Unauthorized":
      return status === 401;
    case "Forbidden":
      return status === 403;
    case "NotFound":
      return status === 404;
    case "BadRequest":
      return status === 400;
    case "Internal":
      return status === 500;
    case "RateLimited":
      return status === 429;
    case "Detail":
      return status === 422;
  }
}

export function createClient(config: RpcConfig): TeleportClient {
  return {
    config,
    async rpc<T, E>(method: HttpMethod, path: string, input: unknown): Promise<RpcResult<T, E>> {
      const fetchImpl = config.fetch ?? globalThis.fetch;

      if (!fetchImpl) {
        const transportError: TransportError = {
          type: "NetworkError",
          message:
            "No fetch implementation available. Provide RpcConfig.fetch or use a runtime with global fetch.",
        };
        config.onError?.({ type: "transport", error: transportError });
        return { kind: "transport", ok: false, transport: transportError };
      }

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

        const response = await fetchImpl(url, init);

        clearTimeout(timeoutId);

        if (!response.ok) {
          const body = await response.text();

          try {
            const parsed = body ? JSON.parse(body) : null;

            if (isAppErrorPayload(parsed) && matchesAppErrorStatus(parsed, response.status)) {
              const result = {
                kind: "error" as const,
                ok: false as const,
                error: parsed as AppError<E>,
              };
              config.onError?.({ type: "app", error: parsed });
              return result;
            }
          } catch {
            // Fall through to a transport-level server failure.
          }

          const transportError: TransportError = {
            type: "ServerError",
            status: response.status,
            body,
          };
          config.onError?.({ type: "transport", error: transportError });
          return { kind: "transport", ok: false, transport: transportError };
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
    },
  };
}
