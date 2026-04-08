import type { Plugin, ViteDevServer } from "vite";
import { resolve } from "node:path";
import { createBindingsWatcher } from "./watcher.js";

/** Configuration options for the teleport-rs Vite plugin. */
export interface TeleportViteOptions {
  /**
   * Path to the directory containing generated TypeScript bindings.
   *
   * @example "../rust-server/bindings"
   */
  bindingsPath: string;

  /**
   * When `true`, runs `cargo run --bin export` at dev-server start
   * to ensure bindings are up-to-date before the first page load.
   *
   * @default false
   */
  generateOnStart?: boolean;
}

/**
 * Vite plugin that watches teleport-rs generated bindings and triggers
 * granular HMR updates when they change.
 */
export function teleportVite(options: TeleportViteOptions): Plugin {
  return {
    name: "teleport-rs",

    async buildStart() {
      if (options.generateOnStart) {
        const { execSync } = await import("node:child_process");
        execSync("cargo run --bin export", { stdio: "inherit" });
      }
    },

    configureServer(server: ViteDevServer) {
      const bindingsDir = resolve(options.bindingsPath);
      const watcher = createBindingsWatcher(server, bindingsDir);

      server.httpServer?.on("close", () => watcher.close());
    },
  };
}
