import assert from "node:assert";
import { describe, it } from "node:test";
import {
  getBindingsWarning,
  normalizeGenerateOnStart,
  runGenerateOnStart,
} from "../index";

type SpawnRunner = (...args: unknown[]) => {
  status?: number | null;
  error?: Error;
};

describe("normalizeGenerateOnStart", () => {
  it("normalizes true to cargo run", () => {
    assert.deepStrictEqual(normalizeGenerateOnStart(true), {
      kind: "argv",
      command: ["cargo", "run"],
    });
  });

  it("normalizes object form with defaults", () => {
    assert.deepStrictEqual(normalizeGenerateOnStart({}), {
      kind: "argv",
      command: ["cargo", "run"],
      cwd: undefined,
      env: undefined,
    });
  });

  it("keeps legacy shell strings", () => {
    assert.deepStrictEqual(normalizeGenerateOnStart("cargo run --bin export"), {
      kind: "shell",
      command: "cargo run --bin export",
    });
  });
});

describe("runGenerateOnStart", () => {
  it("runs argv commands with stdio inherit", () => {
    const calls: Array<{ cmd: string; args: string[]; options: Record<string, unknown> }> = [];

    runGenerateOnStart(
      {
        kind: "argv",
        command: ["cargo", "run", "--bin", "server"],
        cwd: "/tmp/demo",
        env: { TELEPORT_MODE: "export" },
      },
      ((cmd: unknown, args: unknown, options: unknown) => {
        calls.push({
          cmd: String(cmd),
          args: args as string[],
          options: options as Record<string, unknown>,
        });
        return { status: 0 };
      }) as SpawnRunner as never,
    );

    assert.strictEqual(calls.length, 1);
    assert.strictEqual(calls[0]?.cmd, "cargo");
    assert.deepStrictEqual(calls[0]?.args, ["run", "--bin", "server"]);
    assert.deepStrictEqual(calls[0]?.options, {
      stdio: "inherit",
      cwd: "/tmp/demo",
      env: { ...process.env, TELEPORT_MODE: "export" },
    });
  });

  it("throws on non-zero exit", () => {
    assert.throws(
      () =>
        runGenerateOnStart(
          {
            kind: "argv",
            command: ["cargo", "run"],
          },
          (() => ({ status: 2 })) as SpawnRunner as never,
        ),
      /generateOnStart command exited with status 2/,
    );
  });
});

describe("getBindingsWarning", () => {
  it("warns when the bindings directory is missing", () => {
    const warning = getBindingsWarning("/tmp/missing", {
      existsSync: () => false,
      readdirSync: () => [] as string[],
    } as never);

    assert.match(warning ?? "", /Generated bindings directory not found/);
  });

  it("warns when the bindings directory is empty", () => {
    const warning = getBindingsWarning("/tmp/generated", {
      existsSync: () => true,
      readdirSync: () => ["README.md"],
    } as never);

    assert.match(warning ?? "", /No \.ts files found/);
  });
});
