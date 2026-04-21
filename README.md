# teleport-rs

<p align="center">
  <img src="docs/assets/teleport-rs-hero.png" alt="teleport-rs hero banner showing Rust backend procedures flowing through generated types into a TypeScript client" width="1100" />
</p>

Feels like SvelteKit remote functions, but your backend is Rust.

Typed Rust backend functions with generated TypeScript clients for Vite-style frontends.  
No OpenAPI. No manual TypeScript types. No handwritten fetch wrappers.

## What It Is

`teleport-rs` lets you write Rust procedures and call them from TypeScript with end-to-end types:

- Rust input/output types become TypeScript types
- `AppError<T>` becomes a typed client-side error union
- a client is generated for your frontend automatically

It is built for Rust backends and frontend DX, not for public multi-language API contracts.

## Example

**Rust**

```rust
use teleport::{remote, teleport_type, AppError};

#[teleport_type]
struct GetUserInput {
    id: String,
}

#[teleport_type]
struct User {
    id: String,
    name: String,
}

#[remote(query)]
async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> {
    ctx.get_user(&input.id).cloned().ok_or(AppError::NotFound)
}
```

**TypeScript**

```ts
import { createClient } from "@teleport-rs/client";
import { bindClient } from "./generated/client";

const client = createClient({
  baseUrl: "http://localhost:3000",
  credentials: "include",
});

const { users } = bindClient(client);

const result = await users.getUser({ id: "123" });

if (result.ok) {
  console.log(result.data.name);
}
```

## Why Use It

- No schema files like OpenAPI or protobuf
- No manual TypeScript type maintenance
- No route-string-plus-fetch boilerplate in the frontend
- Typed errors flow from Rust to TypeScript
- Works with your existing Axum-based backend

## The Pitch

This is basically:

- SvelteKit remote functions, but your backend lives in Rust
- tRPC-style end-to-end typing, but for a Rust server
- Specta-based type generation plus a real procedure layer

Instead of moving backend logic into `.remote.ts` files, you keep the logic in Rust and call it from the frontend like a typed function.

## When It Makes Sense

Use `teleport-rs` if you have:

- a Vite or Svelte-style frontend
- a Rust backend, especially Axum
- a single product codebase where frontend and backend evolve together
- a strong preference for app-level DX over public API standardization

## When Not To Use It

- Public third-party APIs: use OpenAPI
- Multiple independent client languages: use OpenAPI or gRPC
- Pure SvelteKit backend: use SvelteKit remote functions
- Pure TypeScript full-stack app: use tRPC

## Comparisons

### vs Axum + JSON routes

- Axum alone gives you routes and handlers
- you still write fetch calls manually
- you still keep frontend types in sync yourself

`teleport-rs` removes that glue layer.

### vs Axum + Specta

- Specta gives you shared types
- it does not give you typed procedure calls by itself

`teleport-rs` adds the RPC/client layer on top.

### vs rspc

- similar category: typed RPC across frontend and backend
- `teleport-rs` is more opinionated around a Rust backend plus Vite/Svelte-style frontend flow

### vs tRPC

- tRPC is for a TypeScript backend
- `teleport-rs` is for a Rust backend

### vs SvelteKit remote functions

- remote functions assume SvelteKit is your backend
- `teleport-rs` gives a similar frontend experience when your backend is Rust

## How It Works

1. Annotate Rust procedures with `#[remote(query)]`, `#[remote(command)]`, or `#[remote(form)]`
2. Export bindings from your Rust app
3. Generate `types.ts`, `errors.ts`, and `client.ts`
4. Bind the generated client to a configured runtime client
5. Call backend procedures from TypeScript as typed functions

## Features

- `#[remote(query)]`, `#[remote(command)]`, `#[remote(form)]`
- Typed `Result<T, AppError<E>>` to TypeScript unions
- Procedure-specific typed error details
- Auth support with `#[auth]`
- Vite plugin with binding-aware HMR
- No schema/config layer in normal app usage

## Stack

- Rust
- Axum
- Specta
- TypeScript client runtime
- Vite plugin for generated bindings and HMR

## Status

Early, but usable.

Expect changes while the API settles.

## Getting Started

- Full walkthrough: [docs/getting-started.md](docs/getting-started.md)
- Architecture: [docs/architecture.md](docs/architecture.md)
- Error handling: [docs/error-handling.md](docs/error-handling.md)
- Starter example: [examples/starter/](examples/starter/)
- Demo app: [examples/demo/](examples/demo/)

For local repo development, use npm workspaces:

```bash
npm install
npm run build:js
npm run demo:export
```

## Goal

Make a Rust backend and a TypeScript frontend feel like one app.

Without:

- schema files
- type drift
- handwritten client glue

## License

MIT
