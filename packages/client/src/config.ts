export interface RpcConfig {
  /** Base URL for RPC endpoints (e.g. "http://localhost:3000/rpc"). */
  baseUrl: string;
  /** Request timeout in milliseconds. */
  timeout?: number;
  /** Dynamic headers added to every request. */
  headers?: () => Record<string, string> | Promise<Record<string, string>>;
  /** Fetch credentials mode. Defaults to "include" to forward cookies. */
  credentials?: RequestCredentials;
}

let config: RpcConfig = {
  baseUrl: "/rpc",
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
