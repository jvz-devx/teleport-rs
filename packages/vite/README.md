# @teleport-rs/vite

Vite plugin for [teleport-rs](https://github.com/jvz-devx/teleport-rs).
Watches the directory where your backend export command writes generated
TypeScript bindings and triggers granular HMR updates when they change.

## Install

```bash
npm install -D @teleport-rs/vite
```

You will also want the runtime client:

```bash
npm install @teleport-rs/client
```

## Usage

```typescript
// vite.config.ts
import { defineConfig } from "vite";
import { teleportVite } from "@teleport-rs/vite";

export default defineConfig({
  plugins: [
    teleportVite({
      bindingsPath: "src/lib/api/generated",
    }),
  ],
});
```

The plugin:

- Watches `bindingsPath` for changes to `types.ts`, `client.ts`, and
  `errors.ts` (whichever your backend export command emits).
- Triggers a module graph invalidation so importers HMR without a full
  page reload.
- Warns at dev-server start if `bindingsPath` is missing or empty, with
  a hint to run your export command.

## Options

```typescript
interface TeleportViteOptions {
  /** Path to the directory containing generated TypeScript bindings. */
  bindingsPath: string;

  /**
   * Run a command at dev-server start to regenerate bindings before the
   * first page load.
   *
   * The command should export bindings and exit quickly. Do not point
   * this at a long-running server process.
   *
   * - `true` runs `cargo run` with the current workspace.
   * - A string runs that exact command through the shell (legacy shorthand).
   * - An object runs an explicit argv command, e.g.
   *   `{ command: ["cargo", "run", "--bin", "server", "--", "--export-only"] }`.
   * - `false` (the default) skips the step — use a separate terminal
   *   running `cargo watch -x run`.
   */
  generateOnStart?: boolean | string | {
    command?: string[];
    cwd?: string;
    env?: Record<string, string>;
  };
}
```

## Recommended dev loop

In two terminals:

```bash
# terminal 1: backend server, auto-regenerates bindings on every build
cargo watch -x run

# terminal 2: vite, HMRs on every binding write
npm run dev
```

Or use `generateOnStart` with a short-lived export command:

```ts
teleportVite({
  bindingsPath: "src/lib/api/generated",
  generateOnStart: {
    command: ["cargo", "run", "--bin", "server", "--", "--export-only"],
    cwd: "..",
  },
})
```

The object form is the recommended path. `true` and string values are
kept as shorthand compatibility options.

See [the main repo](https://github.com/jvz-devx/teleport-rs)
for the full walkthrough.
