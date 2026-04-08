import type { AppError, TransportError } from "./types";

export interface RpcConfig {
  /** Base URL for RPC endpoints (e.g. "http://localhost:3000/rpc"). */
  baseUrl: string;
  /** Request timeout in milliseconds. */
  timeout?: number;
  /** Dynamic headers added to every request. */
  headers?: () => Record<string, string> | Promise<Record<string, string>>;
  /** Fetch credentials mode. Defaults to "include" to forward cookies. */
  credentials?: RequestCredentials;
  /** Called on every RPC failure (app error or transport error). Use for global error handling like 401 → redirect. */
  onError?: (error: { type: "app"; error: AppError<unknown> } | { type: "transport"; error: TransportError }) => void;
}

let config: RpcConfig = {
  baseUrl: "",
  timeout: 30_000,
  credentials: "include",
};

/** Update the global RPC client configuration. */
export function configure(opts: Partial<RpcConfig>): void {
  config = { ...config, ...opts };
}

/** Read the current configuration. */
export function getConfig(): Readonly<RpcConfig> {
  return config;
}
