# Architecture

## Crate Structure

```
crates/
├── teleport/          # Main library — re-exports everything users need
│                      # (TeleportRouter, #[remote], #[teleport_type], AppError, AuthedUser)
├── teleport-core/     # Shared types between teleport and teleport-build
│                      # (ProcedureInfo, HttpMethod, ProcedureKind)
├── teleport-macros/   # Proc macro crate — #[remote] and #[teleport_type]
└── teleport-build/    # TypeScript code generation (types.ts, client.ts, errors.ts)
```

**Why the split?** Proc macro crates can only export procedural macros. `teleport-macros` handles the `#[remote]` attribute, which registers procedures via `inventory::submit!`. `teleport-core` exists to share type definitions between `teleport` (runtime) and `teleport-build` (codegen) without circular dependencies. `teleport` re-exports everything so users only need `use teleport::*`.

## npm Packages

```
packages/
├── client/    # @teleport-rs/client — fetch-based RPC client, framework-agnostic
└── vite/      # @teleport-rs/vite — Vite plugin for HMR on generated files
```

## Procedure Collection

teleport-rs uses the [`inventory`](https://docs.rs/inventory) crate for zero-config procedure registration:

1. `#[remote(query)]` expands to a function + an `inventory::submit!(ProcedureRegistration { ... })` call
2. At startup, `TeleportRouter::export()` calls `inventory::iter::<ProcedureRegistration>()` to collect all registered procedures
3. The collected procedures are passed to `teleport-build` for TypeScript generation

This is why export runs as part of the main binary (not `build.rs`) — `inventory` relies on linker-generated data that only exists in the final linked binary.

## Type Generation

Type conversion from Rust to TypeScript is handled by [Specta](https://docs.rs/specta):

1. `#[teleport_type]` expands to `#[derive(specta::Type, serde::Serialize, serde::Deserialize)]`
2. During export, Specta introspects each type's structure
3. `specta-typescript` renders the TypeScript definitions

Generated output:
- `types.ts` — interfaces for all `#[teleport_type]` structs/enums
- `client.ts` — RPC functions grouped by module namespace (e.g., `users.getUser`)
- `errors.ts` — `AppError<T>` union types matching Rust error variants

## Error Architecture

`AppError<T>` is a generic enum with shared variants (`NotFound`, `Unauthorized`, `Internal`, etc.) plus an optional procedure-specific detail type `T`:

```rust
pub enum AppError<T = ()> {
    NotFound,
    Unauthorized,
    Internal(String),
    Detail(T),           // procedure-specific error
    // ...
}
```

On the TypeScript side, this becomes a discriminated union. The `T` parameter flows end-to-end: Rust procedure → generated client → TypeScript call site. Procedures that don't need specific errors use `AppError` (defaults `T = ()`).

## Request Flow

```
Browser → SvelteKit BFF → Rust (teleport-rs)
           (optional)
```

1. Client calls a generated function (e.g., `users.getUser("123")`)
2. The client serializes input and sends an HTTP request (`GET /rpc/users.getUser?id=123`)
3. Axum routes the request to the generated handler
4. Auth middleware extracts and validates the session cookie (if the procedure requires `AuthedUser`)
5. The `#[remote]` handler runs with `&AppState` and returns `Result<T, AppError<E>>`
6. The response is serialized as JSON and returned to the client
7. The client deserializes into `RpcResult<T, E>` — a discriminated union of success, app error, or transport error

Query procedures use GET with `serde_qs` for structured query params. Command procedures use POST with JSON body.

## Auth Middleware

Auth is configured on `TeleportRouter` with a cookie name and validator closure:

```rust
.auth("session", |token, state| async move { state.validate_session(&token) })
```

The validator returns `Option<AuthedUser>`. If the procedure signature includes `AuthedUser`, the middleware rejects unauthenticated requests with 401. `Option<AuthedUser>` makes auth optional.
