# teleport-rs — Project Overview

## What

teleport-rs is an end-to-end type-safe RPC layer between a Rust backend (Axum) and a TypeScript frontend. It generates TypeScript client functions from Rust procedure definitions, providing a remote-function-like developer experience across the language boundary.

## Why

Existing options all have trade-offs:

- **rspc**: The closest match, but unmaintained. Array-based client API (`["getUser", 1]`) is not ergonomic.
- **gRPC + protobuf**: Requires proto files, code generation tooling, and XML-like config. Overkill for a BFF architecture.
- **OpenAPI/Swagger**: YAML config hell. Doesn't fit the "just call a function" DX.
- **REST + ts-rs**: Simple but no transport layer, no type-safe client generation.
- **napi-rs**: In-process only, not for separate services.

teleport-rs fills the gap: write a Rust function, call it from TypeScript with full type safety and zero boilerplate.

## Design Principles

1. **DX above all** — Syntax should feel like calling a local function. Annotate a Rust function, call it from TS. No config files, no YAML, no proto definitions.
2. **Explicit over implicit** — `#[remote(query)]` vs `#[remote(command)]` is explicit intent. `snake_case` → `camelCase` is automatic but overridable.
3. **Rust compiler as guardrail** — If it compiles, the types match across the wire. Specta ensures TypeScript mirrors Rust exactly.
4. **Server-side BFF** — The browser never calls Rust directly. All calls go through a server-side BFF layer (SvelteKit remote functions, Next.js Server Actions, Remix loaders, etc.). This is the security boundary.
5. **Own the glue** — No framework lock-in. The proc macro and generator are simple enough to fork or modify.
6. **JSON only** — No binary serialization. Debuggable in the terminal, in devtools, everywhere. Optimize later if measured.

## Data Flow

```
Browser
  │
  │ BFF server function (e.g., SvelteKit remote, Next.js Server Action)
  │ - Zod validation for UX
  │ - Cookie forwarding
  │
  ▼
BFF Layer (Node)
  │
  │ teleport-rs client (generated TS)
  │ - Type-safe RPC call
  │ - Result<T, E> error handling
  │ - JSON over HTTP
  │
  ▼
Rust Backend (Axum)
  │
  │ teleport-rs server (proc macros)
  │ - #[remote(query/command/form)]
  │ - Axum State injection
  │ - AppError<T> handling
  │
  ▼
Database / Business Logic
```

## Key Differentiators from rspc

| Feature               | rspc                           | teleport-rs                                   |
| --------------------- | ------------------------------ | --------------------------------------------- |
| Maintenance           | Unmaintained                   | Active                                        |
| Client API            | `client.query(["getUser", 1])` | `getUser(1)`                                  |
| Namespace             | No                             | `users.getUser` → `/rpc/users.getUser`        |
| Error types           | Single error enum              | `AppError<T>` with procedure-specific details |
| Transport errors      | Mixed with app errors          | Separate transport vs application errors      |
| Auth                  | Manual                         | Auto-forward cookies + explicit override      |
| Framework integration | None                           | Works with any TS server framework            |
| Naming                | Rust names only                | Auto camelCase + override                     |
| Output                | Single file                    | Split: types.ts, client.ts, errors.ts         |

## Project Scope

teleport-rs consists of:

1. **`teleport-rs`** (Rust crate) — proc macro, router generation, error types, auth middleware, Specta export
2. **`@teleport-rs/client`** (npm package) — generated TS client with Result type, transport handling
3. **`@teleport-rs/vite`** (npm package) — Vite plugin for auto-regeneration and HMR
4. **Monorepo example** — Full-stack demo (Axum + TypeScript frontend)

## Out of Scope (for now)

- Binary serialization
- WebSocket/SSE subscriptions
- Zod schema autogeneration from Specta types
- Router merging (start flat, add later)
- Languages other than TypeScript (Specta already supports Swift; could add later)
