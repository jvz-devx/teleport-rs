# teleport-rs — Design Decisions

This document records all design decisions made during planning, including alternatives considered and rationale.

---

## 1. Procedure Types: `query` / `command` / `form`

**Decision:** Use semantic procedure type annotations inspired by CQRS patterns.

```rust
#[remote(query)]
#[remote(command)]
#[remote(form)]
```

**Alternatives considered:**

- A: `#[remote(query)]` / `#[remote(command)]` / `#[remote(form)]` — explicit, semantic
- B: `#[remote(GET)]` / `#[remote(POST)]` — HTTP methods, loses semantic meaning
- C: `#[remote]` with inference — too ambiguous, can't reliably infer intent from signature

**Rationale:** Explicit is better than implicit. `query` = read, `command` = write, `form` = form submission. These map cleanly to CQRS concepts and HTTP semantics (GET/POST). `form` is identical to `command` at the HTTP level (POST + JSON body), but semantically represents a form action — frameworks like SvelteKit can use this for progressive enhancement, while others treat it as a regular POST endpoint.

---

## 2. Client Boundary: BFF Pattern

**Decision:** The browser never calls Rust directly. All calls go through a Backend-for-Frontend (BFF) layer — SvelteKit, Next.js, Remix, or any server-side framework.

**Rationale:**

- Security: The BFF layer keeps server-only code out of the browser bundle. In SvelteKit this is `.remote.ts` files; in Next.js, Server Actions or API routes; in Remix, loaders/actions.
- Flexibility: The BFF can validate (Zod), transform, cache, and handle auth before calling Rust.
- Colocation: Data loading lives next to the component that uses it, not in a separate API layer.
- Progressive enhancement: `form` procedures can leverage framework-specific progressive enhancement (e.g., SvelteKit's built-in form handling) where available.

---

## 3. Error Handling: `AppError<T>` with Procedure-Specific Details

**Decision:** Use a generic `AppError<T>` enum where `T` defaults to `()` and is replaced with procedure-specific error detail types.

**Alternatives considered:**

- A: Single `AppError` enum — simple but becomes a god enum over time
- B: Per-procedure error types — most precise but boilerplate hell
- C: `AppError<T>` with generic detail — best balance of simplicity and precision

**Rationale:** Most errors are shared (Unauthorized, NotFound, Internal). Procedure-specific errors (InvalidCredentials, AccountLocked) are few and important. Option C gives us both. The `T = ()` default means procedures that don't need specific errors can just use `AppError` with no friction.

---

## 4. Runtime Error Handling: Result Type (No Throwing)

**Decision:** TS client returns `RpcResult<T, E>` — a discriminated union with three variants.

```typescript
type RpcResult<T, E> =
  | { ok: true; data: T }
  | { ok: false; error: AppError<E> } // from Rust
  | { ok: false; transport: TransportError }; // network/protocol
```

**Alternatives considered:**

- A: Throw on error, try/catch — loses type information, TypeScript can't enforce error handling
- B: Result type (chosen) — matches Rust's `Result<T, E>`, TypeScript enforces handling
- C: Discriminated union `{ type: 'ok' | 'error' }` — same concept, different syntax

**Rationale:** Throwing is the JS convention but bad for type safety. The Result pattern forces developers to handle both success and error cases, and the compiler enforces it. This matches the SvelteKit remote function pattern where `form.result` returns similar discriminated unions. Transport vs application error separation makes debugging easier.

---

## 5. Naming: Auto snake_case → camelCase

**Decision:** Rust function names auto-convert to TypeScript camelCase. Optional override with `name = "..."`.

```rust
#[remote(query)] // get_user → users.getUser
async fn get_user(...)

#[remote(query, name = "fetchProfile")] // exact name
async fn get_user_profile(...)
```

**Rationale:** TypeScript uses camelCase. Rust uses snake_case. Auto-conversion is the convention that requires zero mental overhead. The `name` override exists for cases where the auto-converted name isn't desirable or backward compatibility requires a specific name.

---

## 6. Namespacing: Module Path

**Decision:** Procedures are namespaced by their Rust module path.

```rust
// src/api/users.rs
#[remote(query)]
async fn get_user(...) // → /rpc/users.getUser

// src/api/auth.rs
#[remote(query)]
async fn login(...)    // → /rpc/auth.login
```

**Alternatives considered:**

- A: Flat — `/rpc/get_user`, `/rpc/login`
- B: Module path (chosen) — `/rpc/users.getUser`, `/rpc/auth.login`
- C: Flat with optional prefix override

**Rationale:** Namespacing prevents collisions (`users.get` vs `posts.get`), mirrors the file structure (easy to find), and produces readable client code (`users.getUser`, `auth.login`). The dot separator is chosen over slash to distinguish from REST URL paths and match the SvelteKit remote function naming pattern.

---

## 7. Context: Axum State (Not Custom Context Switching)

**Decision:** Procedures receive `&AppState` as their first parameter. Auth is handled via Axum extractors (`AuthedUser`).

**Alternatives considered:**

- A: Axum State — pass `AppState` directly, simple, idiomatic Axum
- B: Custom context switching — separate `UnauthenticatedCtx` → `AuthenticatedCtx`, like rspc
- C: `getRequestEvent()` style — call a function inside the procedure, like SvelteKit

**Rationale:** Option A is simplest, most idiomatic in Rust/Axum, and zero magic. Auth via extractors is already the Axum pattern — no need to reinvent context switching. Option B adds complexity for marginal benefit. Option C feels unidiomatic in Rust and requires thread-local hacks.

---

## 8. Auth: Auto-Forward Cookies + Explicit Override

**Decision:** The BFF layer forwards cookies to Rust by default. Optional `Authorization` header override for specific calls.

**Rationale:** The default case (same-origin BFF → Rust) should "just work" with cookies. The explicit override exists for API keys, service-to-service calls, and cross-origin scenarios. In SvelteKit, cookies are available via `getRequestEvent()`; in Next.js, via `cookies()` from `next/headers`; other frameworks have equivalent mechanisms.

---

## 9. Serialization: JSON Only

**Decision:** All data between the BFF and Rust is JSON. No binary serialization.

**Rationale:**

- The hot path is Browser → BFF, which is always JSON anyway.
- BFF → Rust is localhost, where JSON is sub-millisecond.
- JSON is debuggable in terminal, devtools, and logs.
- Binary serialization doubles testing surface and adds dependencies.
- YAGNI — add binary later if measured bottleneck exists.

---

## 10. Router: Single Flat (Merging Later)

**Decision:** Start with a single flat router. All procedures registered in one module tree. Merging support deferred.

**Rationale:** At 10-20 procedures, a single flat tree organized by file module is perfectly clear. Router merging adds API surface and complexity that isn't needed yet. Can be added in Phase 7 when real projects demand it.

---

## 11. TS Output: Split Files

**Decision:** Generate three files: `types.ts`, `client.ts`, `errors.ts`.

**Alternatives considered:**

- A: Single `generated.ts` — everything in one file, simple but grows large
- B: Split into multiple files (chosen) — better maintainability, cleaner imports

**Rationale:** Projects with 50+ procedures produce large generated files. Splitting by concern (types, client calls, errors) makes it easy to find what you need and reduces merge conflicts (if ever checked in). Barrel re-export via `index.ts` keeps imports clean.

---

## 12. Transport vs Application Errors

**Decision:** Separate transport errors (network, timeout, serialization) from application errors (from Rust) in the `RpcResult` type.

**Rationale:** These errors have different causes and different handling strategies:

- Transport errors → retry, show "network error" UI, exponential backoff
- Application errors → pattern match, show specific error messages, business logic
- Mixing them makes error handling ambiguous and debugging harder

---

## 13. Validation: Both Sides

**Decision:** The BFF layer validates with Zod (UX). Rust validates with serde + business logic (security).

**Rationale:** Defense in depth. Zod gives instant feedback and nice error messages before the network call. Rust gives authoritative validation that can't be bypassed. Neither alone is sufficient — client-side validation can be skipped, server-side validation gives poor UX.

---

## 14. Zod Autogeneration: Deferred

**Decision:** Do not auto-generate Zod schemas from Rust types for now.

**Rationale:** Previous experience with Zod autogeneration has been fragile and error-prone. The Specta → Zod bridge (`specta-zod`) exists but is listed as "planned" and not stable. Handwriting Zod in SvelteKit remote functions is simple, reliable, and gives full control. Revisit when `specta-zod` is production-ready.

---

## 15. Monorepo

**Decision:** Rust backend and SvelteKit frontend in the same repository.

**Rationale:** Type generation from Rust → TS needs to write files into the frontend directory. A monorepo makes this trivial. Separate repos would require CI coordination, published npm packages for generated code, and cross-repo version management. For testing and documentation, having everything together is essential. The export binary writes directly to `../frontend/src/lib/api/generated/`.

---

## 16. Package Name: teleport-rs

**Decision:** Crate name `teleport`, npm scope `@teleport-rs`.

**Rationale:** "Teleport" captures the idea of seamlessly calling a function that appears on the other side (Rust → TS). The `-rs` suffix on npm follows Rust ecosystem convention (like `@tokio-rs`, `@napi-rs`). The crate is just `teleport` on crates.io following Rust naming convention.

---

## 17. Runtime Export Binary Instead of `build.rs`

**Decision:** TypeScript generation is handled by `cargo run --bin export`, not a `build.rs` script.

**Alternatives considered:**

- A: `build.rs` calls `teleport_build::generate()` during `cargo build`
- B: Dedicated export binary (chosen) — `cargo run --bin export`

**Rationale:** `inventory::collect` is a runtime-only mechanism. It relies on linker-generated data structures that are populated when the final binary is linked and executed. Build scripts (`build.rs`) run in a separate compilation context — they cannot access the inventory of `#[remote]` procedures because those procedures haven't been linked into the build script's binary. A dedicated export binary compiles alongside the main application, links against the same crate graph, and can call `inventory::collect` at runtime to discover all registered procedures.

**Dev workflow:** `cargo watch -x 'run --bin export'` watches for Rust changes and regenerates TS bindings automatically.

---

## 18. Query Param Serialization via `qs`

**Decision:** Use `qs.stringify()` for GET request query parameter serialization instead of `URLSearchParams`.

**Alternatives considered:**

- A: `URLSearchParams` with `String(v)` coercion — built-in, no dependencies
- B: `qs` library (chosen) — supports nested objects and arrays

**Rationale:** `URLSearchParams` can only encode flat key-value string pairs. Query procedures may accept inputs with nested objects (e.g., `{ filters: { status: "active" } }`) or arrays (e.g., `{ tags: ["foo", "bar"] }`). Without proper serialization, users would be forced to switch these procedures from `query` to `command` (POST) just to pass structured input — defeating the semantic purpose of queries being read-only GET requests. `qs` serializes nested structures into standard bracket notation (`filters[status]=active&tags[0]=foo`) that Rust's `serde_qs` crate can deserialize on the server side.

---

## 19. Auth: AuthedUser as Explicit Parameter

**Decision:** Authenticated user is accessed via an `AuthedUser` parameter in procedure signatures, extracted from Axum request extensions.

**Alternatives considered:**

- A: `AuthedUser` as explicit function parameter via `FromRequestParts` (chosen)
- B: `current_user: Option<AuthedUser>` field on `AppState` set by middleware
- C: Access via `&Request` extensions inline within the procedure body

**Rationale:** Option A keeps `AppState` immutable and shared — no per-request mutation needed. It's idiomatic Axum (standard `FromRequestParts` extractor). The `#[remote]` macro detects `AuthedUser` or `Option<AuthedUser>` in the parameter list and generates the extraction code. Option B requires per-request cloning/mutation of state which conflicts with `Arc<AppState>`. Option C leaks framework details into business logic.

---

## 20. Rust Edition 2024, MSRV 1.91

**Decision:** Pin Rust edition 2024 with minimum supported Rust version (MSRV) 1.91.

**Rationale:** Edition 2024 (stabilized in Rust 1.85) brings ergonomic improvements including `gen` blocks, `unsafe_op_in_unsafe_fn` as default lint, and refined `use` import resolution. Pinning MSRV to 1.91 ensures all contributors and CI use a consistent toolchain. Workspace-level `edition` and `rust-version` in `Cargo.toml` apply to all crates uniformly.

---

## 21. Export Binary Needs Simplification (Post-Implementation)

**Problem:** The `teleport` ↔ `teleport-build` split created two parallel type hierarchies. Every project needs ~65 lines of boilerplate in `export.rs` to bridge them: manual `HttpMethod` conversion, `Types` collection management, `ProcedureInfo` construction from `ProcedureRegistration`.

**Current state:** See `examples/demo/src/bin/export.rs` for the boilerplate every user must write.

**Decision:** Simplify to a one-liner. Preferred approach: have `teleport-build` depend on `teleport` directly (no actual circular dependency since the cycle only exists through `teleport-macros`), then expose `teleport_build::export_from_inventory(config)` that handles all collection and conversion internally.

**Fallback:** If the direct dependency doesn't work, create a `teleport-core` crate with shared types.

---

## 22. TypeScript Error Handling Needs Convenience Layer (Post-Implementation)

**Problem:** The `RpcResult<T, E>` discriminated union is type-safe but verbose to consume. Every function in `data.remote.ts` has 5-6 identical lines of error unwrapping. The existing `unwrap()` helper loses typed error detail.

**Current state:** See `examples/demo/frontend/src/lib/server/data.remote.ts` for the repetitive pattern.

**Decision:** Add convenience helpers to `@teleport-rs/client`:
- `rpcUnwrap()` — throws a `TeleportError` (extends `Error`) that preserves the full `AppError<E>` for downstream inspection
- `mapError()` — transform errors while keeping the result pattern
- Keep `RpcResult` as the base type for users who prefer explicit handling

---

## 23. Position as Framework-Agnostic (Post-Implementation)

**Problem:** The project was designed around SvelteKit remote functions (`$app/server`), which is experimental. The generated TypeScript client is actually plain `fetch` — framework-agnostic by nature.

**Decision:** Reposition teleport-rs as a general Rust → TypeScript RPC framework. SvelteKit is one integration example, not the primary target. Document usage with:
- SvelteKit remote functions (current example)
- Plain `fetch` / vanilla TS
- React Query / TanStack Query
- Next.js Server Actions

The npm package `@teleport-rs/client` stays framework-agnostic. The Vite plugin remains SvelteKit-compatible but not SvelteKit-exclusive.

---

## 24. Make AuthedUser Generic

**Problem:** `AuthedUser { id: String, email: String }` is a fixed struct that blocks real apps needing roles, permissions, tenant IDs, or other custom fields.

**Decision:** `TeleportRouter` becomes generic over user type `U: Clone + Send + Sync + 'static`. The auth middleware validator closure returns `Option<U>` instead of `Option<AuthedUser>`. The `#[remote]` macro detects auth parameters by trait bound, not by type name — any parameter implementing the required bounds and marked with `#[auth]` (or detected via convention) is treated as the authenticated user.

**Alternatives considered:**

- A: Keep fixed struct, add `HashMap<String, Value>` extras field — half-measure, still not type-safe
- B: Generic user type on router (chosen) — full flexibility, type-safe throughout
- C: Trait object `dyn UserInfo` — loses concrete type, requires downcasting

**Tradeoff:** More complex generics on `TeleportRouter<S, U>`, but necessary for any real-world application. The export binary and proc macro need to be aware of the generic, adding implementation complexity.

---

## 25. Add Global Error Interceptor

**Problem:** No way to handle 401 errors globally (e.g., redirect to login page). Every call site must individually check for unauthorized errors.

**Decision:** Add `onError` callback to `RpcConfig`, called on every failed RPC before returning the `RpcResult`. The callback receives the full error (either `AppError` or `TransportError`) and can perform side effects (redirect, toast notification, logging). The `RpcResult` is still returned to the caller — `onError` is a notification, not an error swallower.

**Alternatives considered:**

- A: `onError` callback (chosen) — simple, composable, doesn't change return types
- B: Middleware/interceptor chain — too complex for the current scope
- C: Global error event emitter — decouples too much, hard to test

**Rationale:** The most common use case is "on 401, redirect to login". This requires exactly one global hook. A callback on config is the simplest mechanism that solves the problem without over-engineering.

---

## 26. Fix Generated Path Format

**Problem:** Generated procedure paths include the `/rpc` prefix (e.g., `"/rpc/auth.login"`), and the default `baseUrl` in `RpcConfig` is also `"/rpc"`. This causes double-prefix requests: `/rpc/rpc/auth.login`.

**Decision:** Change default `baseUrl` to `""` (empty string). Generated paths keep the `/rpc` prefix as-is. Users set `baseUrl` to their server origin when the Rust server is on a different host/port (e.g., `"http://localhost:3000"`).

**Alternatives considered:**

- A: Change default baseUrl to "" (chosen) — minimal change, generated paths stay consistent with Axum routes
- B: Remove /rpc from generated paths, keep baseUrl as "/rpc" — breaks the correspondence between generated paths and Axum route paths
- C: Auto-detect and strip duplicate prefix — fragile, hides the real problem

**Rationale:** The generated paths should match exactly what Axum serves. The `baseUrl` is just a host/origin prefix, not a path prefix. Changing the default to empty string is the smallest fix with the clearest semantics.
