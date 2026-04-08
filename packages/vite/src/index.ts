import type { Plugin, ViteDevServer } from "vite";
import { watch } from "node:fs";
import { resolve, dirname } from "node:path";

export interface TeleportViteOptions {
  /** Path to the generated bindings directory. */
  bindingsPath: string;
  /** Whether to trigger generation on dev server start. */
  generateOnStart?: boolean;
}

export function teleportVite(options: TeleportViteOptions): Plugin {
  return {
    name: "teleport-rs",

    configureServer(server: ViteDevServer) {
      const bindingsDir = dirname(resolve(options.bindingsPath));

      watch(bindingsDir, (_eventType, filename) => {
        if (!filename?.endsWith(".ts")) return;

        const filePath = resolve(bindingsDir, filename);
        const mods = server.moduleGraph.getModulesByFile(filePath);

        if (mods && mods.size > 0) {
          const updates: Array<{
            type: "js-update";
            path: string;
            acceptedPath: string;
            timestamp: number;
          }> = [];
          const timestamp = Date.now();

          for (const mod of mods) {
            server.moduleGraph.invalidateModule(mod);
            for (const importer of mod.importers) {
              server.moduleGraph.invalidateModule(importer);
              if (importer.file) {
                updates.push({
                  type: "js-update",
                  path: importer.url,
                  acceptedPath: importer.url,
                  timestamp,
                });
              }
            }
          }

          if (updates.length > 0) {
            server.ws.send({ type: "update", updates });
            return;
          }
        }

        // Fallback: full reload if module graph resolution fails.
        server.ws.send({ type: "full-reload", path: "*" });
      });
    },
  };
}
