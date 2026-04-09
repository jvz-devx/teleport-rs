# @teleport-rs/vite

Vite plugin for [teleport-rs](https://github.com/refactor-goblin/teleport-rs).
Watches the directory where your Rust server writes its generated
TypeScript bindings and triggers granular HMR updates when they change.

## Install

```bash
bun add -D @teleport-rs/vite         # or: npm install -D / pnpm add -D
```

You will also want the runtime client:

```bash
bun add @teleport-rs/client
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
  `errors.ts` (whichever your Rust server emits via
  `TeleportRouter::export`).
- Triggers a module graph invalidation so importers HMR without a full
  page reload.
- Warns at dev-server start if `bindingsPath` is missing or empty, with
  a hint to run `cargo run`.

## Options

```typescript
interface TeleportViteOptions {
  /** Path to the directory containing generated TypeScript bindings. */
  bindingsPath: string;

  /**
   * Run a command at dev-server start to regenerate bindings before the
   * first page load.
   *
   * - `true` runs `cargo run` with the current workspace.
   * - A string runs that exact command (e.g. `"cargo run --bin export"`).
   * - `false` (the default) skips the step — use a separate terminal
   *   running `cargo watch -x run`.
   */
  generateOnStart?: boolean | string;
}
```

## Recommended dev loop

In two terminals:

```bash
# terminal 1: rust server, auto-regenerates bindings on every build
cargo watch -x run

# terminal 2: vite, HMRs on every binding write
bun run dev
```

Or use `generateOnStart: true` to skip terminal 1 for a one-shot
regeneration at startup.

See [the main repo](https://github.com/refactor-goblin/teleport-rs)
for the full walkthrough.
