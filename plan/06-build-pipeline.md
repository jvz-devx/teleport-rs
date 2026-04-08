# teleport-rs — Build Pipeline and DX

## Overview

The build pipeline must provide a seamless development experience:

1. **Rust compiles** → TypeScript bindings auto-regenerate
2. **SvelteKit dev server runs** → picks up regenerated bindings via HMR
3. **Production build** → both Rust and TS build together, bindings committed or generated at CI

There should be **zero manual steps** in the dev loop.

## Development Flow

```
┌──────────────────────────────────────────────────────────┐
│  Developer Workflow                                       │
│                                                           │
│  1. Edit Rust procedure (add/modify #[remote])            │
│  2. cargo-watch detects change, runs `cargo run --bin     │
│     export` automatically                                 │
│  3. Export binary collects procedures via                  │
│     inventory::collect and generates TS files              │
│  4. TS files written to ../frontend/src/lib/api/generated/│
│  5. Vite detects file change, triggers granular HMR       │
│  6. SvelteKit dev server hot-reloads with new types       │
│  7. If types changed, TS compiler shows errors in IDE     │
│                                                           │
│  Total feedback time: ~2-5 seconds                       │
└──────────────────────────────────────────────────────────┘
```

## Rust Side: Export Binary

TypeScript generation is handled by a dedicated binary (`src/bin/export.rs`), not `build.rs`. This is because `inventory::collect` is a runtime operation — it relies on linker-generated data that is only available when the compiled binary actually runs, not during build scripts.

```rust
// src/bin/export.rs

use std::path::PathBuf;

fn main() {
    let out_dir = std::env::var("TELEPORT_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/src/lib/api/generated"));

    teleport_build::generate(teleport_build::Config {
        output_dir: out_dir,
        namespace_style: teleport_build::NamespaceStyle::ModulePath,
        naming: teleport_build::Naming {
            case: teleport_build::NamingCase::CamelCase,
            ..Default::default()
        },
        include_manifest: cfg!(debug_assertions),
        route_prefix: "/rpc".to_string(),
    }).expect("Failed to generate teleport-rs bindings");
}
```

### When `cargo run --bin export` Runs:

1. The export binary executes
2. `inventory::collect` gathers all `#[remote]` procedures at runtime
3. For each procedure, it generates:
   - Route path (e.g., `/rpc/users.getUser`)
   - HTTP method (GET/POST)
   - Input type signature (via Specta)
   - Output type signature (via Specta)
   - Error type signature (via Specta)
4. Writes three files:
   - `types.ts` — all input/output structs and enums
   - `errors.ts` — `AppError<T>`, `TransportError`, `RpcResult`, procedure-specific error types
   - `client.ts` — namespaced client functions calling the `rpc` helper
5. If files haven't changed, skips write (avoids unnecessary HMR)

### `inventory::collect` Pattern

```rust
// teleport-rs/src/lib.rs

pub use teleport_macros::remote;
pub use teleport_build;

// Registration happens via inventory crate
inventory::collect!(ProcedureRegistration);

pub struct ProcedureRegistration {
    pub name: &'static str,           // "users.getUser"
    pub method: HttpMethod,           // GET or POST
    pub path: &'static str,           // "/rpc/users.getUser"
    pub procedure_type: ProcedureType, // Query, Command, or Form
    pub input_type: specta::DataType, // Specta type info
    pub output_type: specta::DataType,
    pub error_type: specta::DataType,
    pub doc: &'static str,            // Doc comment from Rust
}
```

The `#[remote]` proc macro generates an `inventory::submit!` block that registers each procedure at compile time.

## TypeScript Side: Vite Plugin

```typescript
// @teleport-rs/vite/src/index.ts

import type { Plugin } from "vite";
import { watch } from "fs";
import { resolve, dirname } from "path";

export interface TeleportViteOptions {
  /** Path to the Rust project's generated bindings */
  bindingsPath: string;
  /** Path to write processed bindings in the SvelteKit project */
  outputPath: string;
  /** Whether to run generation on startup */
  generateOnStart: boolean;
}

export function teleportVite(options: TeleportViteOptions): Plugin {
  return {
    name: "teleport-rs",

    async buildStart() {
      if (options.generateOnStart) {
        // Trigger cargo build to regenerate bindings
        // (or just watch for file changes)
      }
    },

    configureServer(server) {
      // Watch the generated bindings directory for changes
      const bindingsDir = dirname(options.bindingsPath);

      watch(bindingsDir, (eventType, filename) => {
        if (filename?.endsWith(".ts")) {
          const filePath = resolve(bindingsDir, filename);

          // Granular HMR: invalidate only modules that import from generated/
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
              // Also invalidate importers (modules that import this generated file)
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

          // Fallback: full reload if module graph resolution fails
          server.ws.send({ type: "full-reload", path: "*" });
        }
      });
    },
  };
}
```

### SvelteKit Configuration

```typescript
// svelte.config.js

import adapter from "@sveltejs/adapter-auto";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { teleportVite } from "@teleport-rs/vite";

export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter(),
    vite: {
      plugins: [
        teleportVite({
          bindingsPath: "../rust-server/bindings",
          outputPath: "src/lib/api/generated",
          generateOnStart: true,
        }),
      ],
    },
  },
};
```

## TypeScript Generation Pipeline (Detail)

```
┌──────────────────────────────────────┐
│  Rust Source Code                     │
│  ┌─────────────────────────────────┐  │
│  │ #[remote(query)]                │  │
│  │ async fn get_user(             │  │
│  │   ctx: &AppState,              │  │
│  │   id: String                   │  │
│  │ ) -> Result<User, AppError<…>> │  │
│  └─────────────────────────────────┘  │
│                                       │
│  ┌─────────────────────────────────┐  │
│  │ #[derive(TeleportType)]         │  │
│  │ struct User { id, name, email } │  │
│  └─────────────────────────────────┘  │
└──────────┬───────────────────────────┘
           │ cargo run --bin export
           ▼
┌──────────────────────────────────────┐
│  inventory::collect!(Procedure…)      │
│  + Specta type introspection         │
└──────────┬───────────────────────────┘
           │ teleport_build::generate()
           ▼
┌──────────────────────────────────────┐
│  Generated TypeScript                │
│                                      │
│  types.ts:                           │
│    export interface User { … }       │
│    export interface LoginRequest { …}│
│                                      │
│  errors.ts:                          │
│    export type AppError<T> = …       │
│    export type TransportError = …    │
│    export type RpcResult<T, E> = …   │
│    export type GetUserError = …      │
│                                      │
│  client.ts:                          │
│    export const users = {            │
│      getUser: (id) => rpc(…)        │
│    }                                 │
└──────────┬───────────────────────────┘
           │ Vite HMR detects change
           ▼
┌──────────────────────────────────────┐
│  SvelteKit Remote Functions          │
│  (data.remote.ts)                    │
│                                      │
│  import { users } from              │
│    '$lib/api/generated/client'      │
│                                      │
│  export const getUserRemote =       │
│    query(z.string(), async (id) => {│
│      const result = await           │
│        users.getUser(id);           │
│      // ...handle result             │
│    });                               │
└──────────────────────────────────────┘
```

## Production Build

```
1. CI pipeline runs cargo build --release (compiles server + export binary)
2. cargo run --bin export --release generates fresh TS bindings
3. Bindings written to ../frontend/src/lib/api/generated/
4. npm run build (SvelteKit) uses the fresh bindings
5. TypeScript compiler validates all types match
6. If types mismatch, CI fails
```

### CI / CD Pipeline

```yaml
# .github/workflows/build.yml

name: Build & Deploy

on: [push]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install Node
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Build Rust server
        run: cargo build --release
        working-directory: rust-server

      - name: Generate TypeScript bindings
        run: cargo run --bin export --release
        working-directory: rust-server

      - name: Install frontend deps
        run: npm ci
        working-directory: frontend

      - name: Type check frontend
        run: npx svelte-check --tsconfig ./tsconfig.json
        working-directory: frontend

      - name: Build frontend
        run: npm run build
        working-directory: frontend

      # Deploy both
```

## Monorepo Structure

```
my-project/
├── rust-server/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── bin/
│   │   │   └── export.rs           # ← runs teleport_build::generate()
│   │   ├── main.rs
│   │   ├── state.rs
│   │   ├── auth.rs
│   │   └── api/
│   │       ├── mod.rs
│   │       ├── users.rs
│   │       ├── auth.rs
│   │       └── posts.rs
│   └── bindings/                   # ← temporary, gitignored
│       └── (generated at build time)
│
├── frontend/
│   ├── package.json
│   ├── svelte.config.js            # ← teleportVite plugin
│   ├── src/
│   │   ├── lib/
│   │   │   ├── api/
│   │   │   │   ├── generated/      # ← auto-generated by teleport-rs
│   │   │   │   │   ├── types.ts
│   │   │   │   │   ├── client.ts
│   │   │   │   │   └── errors.ts
│   │   │   │   ├── config.ts        # ← configure rpc client
│   │   │   │   └── index.ts         # ← barrel re-exports
│   │   │   └── server/
│   │   │       └── data.remote.ts   # ← handwritten remote functions
│   │   └── routes/
│   │       └── +page.svelte
│   └── vite.config.ts
│
├── .gitignore                      # ← ignore bindings/ and generated/
└── README.md
```

## Configuring Generated Output Path

The export binary reads `TELEPORT_OUTPUT_DIR` as an environment variable override:

```bash
# Default: ../frontend/src/lib/api/generated (relative to Cargo.toml)
cargo run --bin export

# Override for CI or custom setups
TELEPORT_OUTPUT_DIR=/path/to/frontend/src/lib/api/generated cargo run --bin export
```

## `@teleport-rs/client` Package

```json
{
  "name": "@teleport-rs/client",
  "version": "0.1.0",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "exports": {
    ".": {
      "import": "./dist/index.js",
      "types": "./dist/index.d.ts"
    }
  },
  "dependencies": {
    "qs": "^6.13.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0"
  },
  "peerDependencies": {
    "typescript": ">=5.0.0"
  }
}
```

Single runtime dependency: `qs` for query string serialization of nested objects and arrays.

## `@teleport-rs/vite` Package

```json
{
  "name": "@teleport-rs/vite",
  "version": "0.1.0",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "dependencies": {
    "vite": "^5.0.0 || ^6.0.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0",
    "svelte": "^5.0.0"
  }
}
```

## Tailwind for Regeneration

For developers not using Vite (e.g., other editors), `cargo-watch` can trigger regeneration:

```bash
# Terminal 1: Watch Rust changes and regenerate bindings
cargo watch -x 'run --bin export'

# Terminal 2: SvelteKit dev server (picks up generated file changes automatically)
npm run dev
```
