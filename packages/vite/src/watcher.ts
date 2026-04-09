import { watch, type FSWatcher } from "node:fs";
import { resolve } from "node:path";
import type { ViteDevServer } from "vite";

/**
 * Creates a file watcher on the bindings directory that triggers
 * granular HMR updates when generated TypeScript files change.
 *
 * Falls back to a full page reload if the changed file isn't
 * tracked in the Vite module graph.
 */
export function createBindingsWatcher(
  server: ViteDevServer,
  bindingsDir: string,
): FSWatcher {
  return watch(bindingsDir, (_eventType, filename) => {
    if (!filename?.endsWith(".ts")) return;

    const filePath = resolve(bindingsDir, filename);
    const mods = server.moduleGraph.getModulesByFile(filePath);

    if (mods && mods.size > 0) {
      const timestamp = Date.now();
      const updates: Array<{
        type: "js-update";
        path: string;
        acceptedPath: string;
        timestamp: number;
      }> = [];

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

    // Fallback: full reload if the file isn't in the module graph yet.
    server.ws.send({ type: "full-reload", path: "*" });
  });
}
