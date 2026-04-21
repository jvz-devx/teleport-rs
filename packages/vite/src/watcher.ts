import { resolve } from "node:path";
import type { ViteDevServer } from "vite";

type UpdateMessage = {
  type: "js-update";
  path: string;
  acceptedPath: string;
  timestamp: number;
};

/**
 * Creates a watcher on the bindings directory that triggers
 * granular HMR updates when generated TypeScript files change.
 *
 * Falls back to a full page reload if the changed file isn't
 * tracked in the Vite module graph.
 */
export function createBindingsWatcher(
  server: ViteDevServer,
  bindingsDir: string,
  options?: { debounceMs?: number },
): () => void {
  const debounceMs = options?.debounceMs ?? 25;
  const pending = new Set<string>();
  let needsFullReload = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flush = () => {
    timer = undefined;

    if (needsFullReload) {
      pending.clear();
      needsFullReload = false;
      server.ws.send({ type: "full-reload", path: "*" });
      return;
    }

    const updates = collectUpdates(server, [...pending]);
    pending.clear();

    if (updates.length > 0) {
      server.ws.send({ type: "update", updates });
      return;
    }

    server.ws.send({ type: "full-reload", path: "*" });
  };

  const scheduleFlush = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(flush, debounceMs);
  };

  const onChange = (filePath: string) => {
    if (!filePath.endsWith(".ts")) return;
    pending.add(resolve(bindingsDir, filePath));
    scheduleFlush();
  };

  const onAddOrUnlink = (filePath: string) => {
    if (!filePath.endsWith(".ts")) return;
    needsFullReload = true;
    pending.add(resolve(bindingsDir, filePath));
    scheduleFlush();
  };

  server.watcher.add(bindingsDir);
  server.watcher.on("change", onChange);
  server.watcher.on("add", onAddOrUnlink);
  server.watcher.on("unlink", onAddOrUnlink);

  return () => {
    if (timer) clearTimeout(timer);
    server.watcher.off("change", onChange);
    server.watcher.off("add", onAddOrUnlink);
    server.watcher.off("unlink", onAddOrUnlink);
    void server.watcher.unwatch(bindingsDir);
  };
}

export function collectUpdates(
  server: Pick<ViteDevServer, "moduleGraph">,
  filePaths: string[],
): UpdateMessage[] {
  const timestamp = Date.now();
  const updates = new Map<string, UpdateMessage>();

  for (const filePath of filePaths) {
    const mods = server.moduleGraph.getModulesByFile(filePath);
    if (!mods || mods.size === 0) continue;

    for (const mod of mods) {
      server.moduleGraph.invalidateModule(mod);
      for (const importer of mod.importers) {
        server.moduleGraph.invalidateModule(importer);
        if (importer.file) {
          updates.set(importer.url, {
            type: "js-update",
            path: importer.url,
            acceptedPath: importer.url,
            timestamp,
          });
        }
      }
    }
  }

  return [...updates.values()];
}
