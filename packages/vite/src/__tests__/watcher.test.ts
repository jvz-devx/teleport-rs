import assert from "node:assert";
import { describe, it } from "node:test";
import { collectUpdates, createBindingsWatcher } from "../watcher";

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

type FakeModule = {
  file?: string;
  url: string;
  importers: Set<FakeModule>;
};

function createFakeWatcher() {
  const listeners = new Map<string, Set<(filePath: string) => void>>();

  return {
    added: [] as string[],
    unwatchCalls: [] as string[],
    on(event: string, cb: (filePath: string) => void) {
      const set = listeners.get(event) ?? new Set<(filePath: string) => void>();
      set.add(cb);
      listeners.set(event, set);
      return this;
    },
    off(event: string, cb: (filePath: string) => void) {
      listeners.get(event)?.delete(cb);
      return this;
    },
    add(path: string) {
      this.added.push(path);
      return this;
    },
    unwatch(path: string) {
      this.unwatchCalls.push(path);
      return Promise.resolve();
    },
    emit(event: string, filePath: string) {
      for (const cb of listeners.get(event) ?? []) {
        cb(filePath);
      }
    },
  };
}

describe("collectUpdates", () => {
  it("invalidates modules and importers and returns deduped update messages", () => {
    const importer: FakeModule = {
      file: "/tmp/app.ts",
      url: "/src/app.ts",
      importers: new Set(),
    };
    const mod: FakeModule = {
      file: "/tmp/generated/client.ts",
      url: "/src/generated/client.ts",
      importers: new Set([importer]),
    };
    const invalidated: string[] = [];

    const updates = collectUpdates(
      {
        moduleGraph: {
          getModulesByFile(filePath: string) {
            return filePath === "/tmp/generated/client.ts" ? new Set([mod]) : undefined;
          },
          invalidateModule(module: FakeModule) {
            invalidated.push(module.url);
          },
        },
      } as never,
      ["/tmp/generated/client.ts", "/tmp/generated/client.ts"],
    );

    assert.strictEqual(updates.length, 1);
    assert.strictEqual(updates[0]?.path, "/src/app.ts");
    assert.deepStrictEqual(invalidated, [
      "/src/generated/client.ts",
      "/src/app.ts",
      "/src/generated/client.ts",
      "/src/app.ts",
    ]);
  });
});

describe("createBindingsWatcher", () => {
  it("sends one debounced update for multiple generated changes", async () => {
    const watcher = createFakeWatcher();
    const wsMessages: unknown[] = [];
    const importer: FakeModule = {
      file: "/tmp/app.ts",
      url: "/src/app.ts",
      importers: new Set(),
    };
    const mod: FakeModule = {
      file: "/tmp/generated/client.ts",
      url: "/src/generated/client.ts",
      importers: new Set([importer]),
    };

    const cleanup = createBindingsWatcher(
      {
        watcher,
        ws: {
          send(message: unknown) {
            wsMessages.push(message);
          },
        },
        moduleGraph: {
          getModulesByFile(filePath: string) {
            return filePath.startsWith("/tmp/generated/") ? new Set([mod]) : undefined;
          },
          invalidateModule() {},
        },
      } as never,
      "/tmp/generated",
      { debounceMs: 1 },
    );

    watcher.emit("change", "/tmp/generated/client.ts");
    watcher.emit("change", "/tmp/generated/types.ts");
    await wait(10);

    assert.strictEqual(watcher.added[0], "/tmp/generated");
    assert.strictEqual(wsMessages.length, 1);
    assert.deepStrictEqual(wsMessages[0], {
      type: "update",
      updates: [wsMessages[0] && (wsMessages[0] as { updates: unknown[] }).updates[0]],
    });

    cleanup();
    assert.deepStrictEqual(watcher.unwatchCalls, ["/tmp/generated"]);
  });

  it("falls back to full reload for add/unlink events", async () => {
    const watcher = createFakeWatcher();
    const wsMessages: unknown[] = [];

    createBindingsWatcher(
      {
        watcher,
        ws: {
          send(message: unknown) {
            wsMessages.push(message);
          },
        },
        moduleGraph: {
          getModulesByFile() {
            return undefined;
          },
          invalidateModule() {},
        },
      } as never,
      "/tmp/generated",
      { debounceMs: 1 },
    );

    watcher.emit("add", "/tmp/generated/client.ts");
    await wait(10);

    assert.deepStrictEqual(wsMessages, [
      { type: "full-reload", path: "*" },
    ]);
  });
});
