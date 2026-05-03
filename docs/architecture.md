# Architecture

## Crate Structure

```
crates/
├── teleport/          # Main library — re-exports everything users need
│                      # (TeleportRouter, #[remote], #[teleport_type], AppError, AuthedUser)
├── teleport-core/     # Rust-only runtime registration metadata
│                      # (ProcedureRegistration, HttpMethod, ProcedureType)
├── teleport-contract/ # Language-neutral contract bundle for codegen/conformance
├── teleport-macros/   # Proc macro crate — #[remote] and #[teleport_type]
└── teleport-build/    # TypeScript code generation from ContractBundle
```

**Why the split?** Proc macro crates can only export procedural macros. `teleport-macros` handles the `#[remote]` attribute, which registers procedures via `inventory::submit!`. `teleport-core` stays Rust-specific and only carries the runtime metadata needed to mount handlers. `teleport-contract` is the portable boundary: a versioned bundle of procedure descriptors and type definitions. `teleport-build` now consumes that contract instead of reaching directly into Rust discovery. `teleport` still re-exports everything users need for the Rust happy path.

## npm Packages

```
packages/
├── client/    # @teleport-rs/client — fetch-based RPC client, framework-agnostic
└── vite/      # @teleport-rs/vite — Vite plugin for HMR on generated files
```

## Additional Implementations

The repository also contains first-party non-Rust implementations built against the same contract boundary.

```
dotnet/
├── src/Teleport.Net             # Attributes, result/error types, contract export
├── src/Teleport.Net.AspNetCore  # ASP.NET Core discovery, binding, auth, manifest, runtime
├── examples/Teleport.Net.Demo   # Reference demo app
└── tests/                       # Exporter/runtime parity coverage
```

```
go/
├── teleport/          # Contract types, result/error helpers, procedure builders
├── teleporthttp/      # net/http runtime, auth hook, manifest endpoint
└── examples/demo/     # Reference demo app and contract export
```

The intended architectural cutoff is:

- Rust internals stay Rust-specific
- language-neutral behavior lives in `teleport-contract`
- TypeScript generation consumes the contract bundle, not Rust-only discovery
- `.NET`, Go, and future implementations match the shared contract and externally visible wire semantics

## Procedure Collection

teleport-rs uses the [`inventory`](https://docs.rs/inventory) crate for zero-config procedure registration:

1. `#[remote(query)]` expands to a function + an `inventory::submit!(ProcedureRegistration { ... })` call
2. At export time, `TeleportRouter::contract()` / `TeleportRouter::export()` call `inventory::iter::<ProcedureRegistration>()` to collect all registered procedures
3. Rust-specific metadata and Specta-discovered types are mapped into a `teleport-contract::ContractBundle`
4. `teleport-build` consumes that contract bundle to generate TypeScript bindings

This is why export runs as part of the main binary (not `build.rs`) — `inventory` relies on linker-generated data that only exists in the final linked binary.

## Type Generation

Type conversion from Rust to TypeScript is handled by [Specta](https://docs.rs/specta):

1. `#[teleport_type]` expands to `#[derive(specta::Type, serde::Serialize, serde::Deserialize)]`
2. During export, Specta introspects each type's structure
3. `specta-typescript` renders the TypeScript definitions

Generated output:

- `types.ts` — interfaces for all `#[teleport_type]` structs/enums
- `client.ts` — explicit-client RPC helpers plus `bindClient(client)` namespaced wrappers (e.g., `users.getUser`)
- `errors.ts` — `AppError<T>` union types matching Rust error variants

The same export pass can also write `teleport.contract.json` via `TeleportRouter::export_contract(...)`. That file is the handoff point for `.NET`, Go, and any future implementation: native backends produce the same contract shape, then `teleport-cli generate-ts` produces the frontend bindings.

## Cross-language implementations

The contract boundary is no longer theoretical. The repo currently has:

- Rust in `crates/`
- `.NET` in `dotnet/`
- Go in `go/`

All implementations are expected to align on:

- contract schema shape (`teleport-contract::ContractBundle`)
- route shape (`/rpc/{namespace}.{method}`)
- request encoding semantics for query / command / form procedures
- tagged `AppError` response envelopes and HTTP status mapping

Everything above that boundary remains native to the host stack. Rust keeps proc macros, `inventory`, Axum integration, and Specta-backed export internals. `.NET` keeps attribute discovery, ASP.NET endpoint mapping, and `System.Text.Json`-based type export/runtime binding. Go currently uses explicit procedure registration and a small `net/http` runtime instead of discovery or macro-based authoring.

The authoring syntax is deliberately not identical across languages:

- Rust uses `#[remote(query)]`, `#[remote(command)]`, and `#[remote(form)]` proc macros.
- `.NET` uses `[TeleportModule]`, `[TeleportQuery]`, `[TeleportCommand]`, `[TeleportForm]`, and static procedure methods.
- Go uses explicit builders like `teleport.QueryFor[TIn, TOut](...)` and `teleport.QueryWithErrorFor[TIn, TOut, TErr](...)`.

That syntax should stay idiomatic per platform. The invariant is the exported contract and wire behavior, not the internal implementation shape.

## Parity Gates

Cross-language parity is enforced at the demo contract boundary:

1. `npm run demo:export` exports the Rust demo contract.
2. `npm run demo:export:dotnet` exports the `.NET` demo contract and regenerates the frontend bindings through `teleport-cli`.
3. `npm run demo:export:go` exports the Go demo contract and regenerates the frontend bindings through `teleport-cli`.
4. `npm run contracts:parity` compares the Rust, `.NET`, and Go demo contracts.

CI also builds/tests each host implementation independently, then runs frontend check/build against bindings generated from each backend. Passing parity means the demo contracts match; it does not automatically prove every future API surface is equivalent, so new contract features should add coverage in Rust, `.NET`, Go, and the shared TypeScript generator.

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

On the TypeScript side, this becomes a discriminated union. The `T` parameter flows end-to-end: backend procedure → generated client → TypeScript call site. Procedures that don't need specific errors use `AppError` (defaults `T = ()`).

## Request Flow

```
Browser → SvelteKit BFF → native backend
           (optional)
```

1. Client calls a bound generated function (e.g., `users.getUser({ id: "123" })`)
2. The client serializes input and sends an HTTP request (`GET /rpc/users.getUser?id=123`)
3. The host runtime routes the request to the registered procedure
4. Auth middleware extracts and validates the session cookie if the procedure requires auth
5. The native handler runs and returns a success payload or `AppError<E>`
6. The response is serialized as JSON and returned to the client
7. The client deserializes into `RpcResult<T, E>` — a discriminated union of success, app error, or transport error

Query procedures use GET with structured query params. Command procedures use POST with a JSON body. Form procedures use POST with form body semantics. The Rust runtime implements this with Axum extractors (`serde_qs` for query and `FormOrJson` for form); `.NET` and Go implement matching wire behavior in their own runtime binders.

## Auto-applied safety layers

`TeleportRouter::mount()` wraps every router it returns in two tower layers before the final `with_state`:

1. **`tower_http::limit::RequestBodyLimitLayer`**, configured from `DEFAULT_BODY_LIMIT` (2 MiB) unless overridden with `.body_limit(bytes)` or removed with `.no_body_limit()`. `mount()` also applies axum's `DefaultBodyLimit::max(bytes)` so `Json`/`Form`/`Bytes` extractors honour the same limit (axum's extractors otherwise enforce their own internal 2 MiB default). Oversized requests are rejected with `413 Payload Too Large` before any handler runs.
2. **`tower_http::catch_panic::CatchPanicLayer`**, wrapping the router so that a panic in any `#[remote]` handler returns a generic JSON `500` and logs the payload to stderr instead of crashing the process. Opt out with `.no_catch_panic()` if you want panics to propagate (for example, under a supervisor).

The layers are opt-out, not opt-in — the escape hatches are deliberately loud (`no_body_limit`, `no_catch_panic`) so a reviewer can spot them in a diff. See [`security.md`](security.md) for the production rationale and [`feature-flags.md`](feature-flags.md) for how the manifest endpoint interacts with these layers.

## Auth Middleware

Auth is configured on `TeleportRouter` with a cookie name and validator closure. The validator is generic — it returns `Option<U>` for any user type `U: Clone + Send + Sync + 'static`:

```rust
.auth("session", |token, state| async move { state.validate_session(&token) })
```

If the procedure signature includes a parameter with `#[auth]`, the middleware rejects unauthenticated requests with 401. The built-in `AuthedUser` type works by convention (no attribute needed). Use `Option<T>` for optional auth.
