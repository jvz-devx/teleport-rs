import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import type { Plugin, ViteDevServer } from "vite";
import { createBindingsWatcher } from "./watcher.js";

export interface GenerateOnStartOptions {
  /** Command argv to run before the dev server starts. Defaults to `["cargo", "run"]`. */
  command?: string[];
  /** Optional working directory for the command. */
  cwd?: string;
  /** Optional environment overrides for the command. */
  env?: Record<string, string>;
}

/** Configuration options for the teleport-rs Vite plugin. */
export interface TeleportViteOptions {
  /**
   * Path to the directory containing generated TypeScript bindings.
   *
   * @example "../rust-server/bindings"
   */
  bindingsPath: string;

  /**
   * Run a short-lived command at dev-server start to regenerate bindings
   * before the first page load.
   *
   * - `true` runs `cargo run` with the current workspace.
   * - A string runs that exact command through the shell (legacy shorthand).
   * - An object runs an explicit argv command, e.g.
   *   `{ command: ["cargo", "run", "--bin", "server", "--", "--export-only"] }`.
   * - `false` (the default) skips the step — use a separate terminal
   *   running `cargo watch -x run`.
   *
   * @default false
   */
  generateOnStart?: boolean | string | GenerateOnStartOptions;
}

type NormalizedGenerateOnStart =
  | {
    kind: "argv";
    command: string[];
    cwd?: string;
    env?: Record<string, string>;
  }
  | {
    kind: "shell";
    command: string;
  };

export function normalizeGenerateOnStart(
  generateOnStart: TeleportViteOptions["generateOnStart"],
): NormalizedGenerateOnStart | null {
  if (!generateOnStart) return null;
  if (generateOnStart === true) {
    return {
      kind: "argv",
      command: ["cargo", "run"],
    };
  }
  if (typeof generateOnStart === "string") {
    return {
      kind: "shell",
      command: generateOnStart,
    };
  }

  return {
    kind: "argv",
    command: generateOnStart.command ?? ["cargo", "run"],
    cwd: generateOnStart.cwd,
    env: generateOnStart.env,
  };
}

export function getBindingsWarning(
  bindingsDir: string,
  fs = { existsSync, readdirSync },
): string | null {
  if (!fs.existsSync(bindingsDir)) {
    return (
      "[teleport-rs] Generated bindings directory not found: " +
      bindingsDir +
      '\n  Run your export command to generate TypeScript bindings.'
    );
  }

  const tsFiles = fs.readdirSync(bindingsDir).filter((f) => f.endsWith(".ts"));
  if (tsFiles.length === 0) {
    return (
      "[teleport-rs] No .ts files found in " +
      bindingsDir +
      '\n  Run your export command to generate TypeScript bindings.'
    );
  }

  return null;
}

export function runGenerateOnStart(
  command: NormalizedGenerateOnStart,
  run = spawnSync,
): void {
  const result =
    command.kind === "argv"
      ? run(command.command[0] ?? "cargo", command.command.slice(1), {
        stdio: "inherit",
        cwd: command.cwd,
        env: command.env ? { ...process.env, ...command.env } : process.env,
      })
      : run(command.command, {
        stdio: "inherit",
        shell: true,
      });

  if (result.error) {
    throw new Error(
      `[teleport-rs] Failed to run generateOnStart command: ${formatGenerateOnStart(command)}\n${result.error.message}`,
    );
  }

  if (typeof result.status === "number" && result.status !== 0) {
    throw new Error(
      `[teleport-rs] generateOnStart command exited with status ${result.status}: ${formatGenerateOnStart(command)}`,
    );
  }
}

function formatGenerateOnStart(command: NormalizedGenerateOnStart): string {
  return command.kind === "argv" ? command.command.join(" ") : command.command;
}

/**
 * Vite plugin that watches teleport-rs generated bindings and triggers
 * granular HMR updates when they change.
 */
export function teleportVite(options: TeleportViteOptions): Plugin {
  return {
    name: "teleport-rs",
    apply: "serve",

    configureServer(server: ViteDevServer) {
      const bindingsDir = resolve(options.bindingsPath);
      const generateOnStart = normalizeGenerateOnStart(options.generateOnStart);

      if (generateOnStart) {
        runGenerateOnStart(generateOnStart);
      }

      const warning = getBindingsWarning(bindingsDir);
      if (warning) {
        server.config.logger.warn(warning);
      }

      const cleanupWatcher = createBindingsWatcher(server, bindingsDir);
      server.httpServer?.on("close", () => cleanupWatcher());
    },
  };
}
