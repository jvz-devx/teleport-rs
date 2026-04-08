import type { Plugin, ViteDevServer } from "vite";
import { existsSync, readdirSync } from "node:fs";
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
   * When `true`, runs `cargo run` at dev-server start to ensure
   * bindings are up-to-date before the first page load. Pass a string
   * to override the command (e.g. `"cargo run --bin export"`).
   *
   * @default false
   */
  generateOnStart?: boolean | string;
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
        const cmd =
          typeof options.generateOnStart === "string"
            ? options.generateOnStart
            : "cargo run";
        execSync(cmd, { stdio: "inherit" });
      }
    },

    configureServer(server: ViteDevServer) {
      const bindingsDir = resolve(options.bindingsPath);

      // Check for stale/missing bindings
      if (!existsSync(bindingsDir)) {
        server.config.logger.warn(
          "[teleport-rs] Generated bindings directory not found: " +
            bindingsDir +
            '\n  Run "cargo run" to generate TypeScript bindings.',
        );
      } else {
        const tsFiles = readdirSync(bindingsDir).filter((f) =>
          f.endsWith(".ts"),
        );
        if (tsFiles.length === 0) {
          server.config.logger.warn(
            "[teleport-rs] No .ts files found in " +
              bindingsDir +
              '\n  Run "cargo run" to generate TypeScript bindings.',
          );
        }
      }

      const watcher = createBindingsWatcher(server, bindingsDir);

      server.httpServer?.on("close", () => watcher.close());
    },
  };
}
