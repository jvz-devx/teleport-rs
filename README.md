# teleport-rs

[![CI](https://github.com/jvz-devx/teleport-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/jvz-devx/teleport-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.93+](https://img.shields.io/badge/rust-1.93+-orange.svg)](https://www.rust-lang.org)

Write a Rust function, call it from TypeScript. Full type safety, zero config files.

## What it does

teleport-rs generates a fully typed TypeScript client from your Rust API. Annotate functions with `#[remote]`, and get type-safe RPC calls with end-to-end error typing — no OpenAPI specs, no proto files, no manual type definitions.

## Quick Example

**Rust:**

```rust
use teleport::{remote, teleport_type, AppError, TeleportRouter};

#[teleport_type]
struct User {
    id: String,
    name: String,
}

#[remote(query)]
async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError> {
    ctx.get_user(&id).cloned().ok_or(AppError::NotFound)
}
```

**Generated TypeScript:**

```typescript
import { users } from "./generated/client";

const result = await users.getUser("123");
if (result.ok) {
    console.log(result.data.name); // fully typed!
}
```

## Features

- `#[remote(query)]` — GET endpoints, input from query params
- `#[remote(command)]` — POST endpoints, JSON body
- `#[remote(form)]` — POST endpoints, accepts both form-urlencoded and JSON
- Typed errors with `AppError<T>` flowing to TypeScript
- Generic auth with custom user types via `#[auth]` attribute
- Vite plugin with granular HMR
- Single binary — server and export in one

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
teleport = { version = "0.1", features = ["export"] }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

For your frontend (bun, pnpm, or npm — all work):

```bash
bun add @teleport-rs/client @teleport-rs/vite
```

## Getting Started

See [docs/getting-started.md](docs/getting-started.md) for a full walkthrough.

- Start from the minimal example: [`examples/starter/`](examples/starter/)
- Fuller walkthrough with auth, modules, and typed errors: [`examples/demo/`](examples/demo/)

## Documentation

- [Architecture](docs/architecture.md) — design decisions and crate structure
- [Error handling](docs/error-handling.md) — `AppError<T>` variants, typed details, `try_auth`
- [Feature flags](docs/feature-flags.md) — `export` and `debug-manifest`
- [Security](docs/security.md) — production checklist (body limits, panic recovery, CORS)

## License

MIT
