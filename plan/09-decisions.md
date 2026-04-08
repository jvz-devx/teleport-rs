# teleport-rs — Design Decisions

This document records all design decisions made during planning, including alternatives considered and rationale.

---

## 1. Procedure Types: `query` / `command` / `form`

**Decision:** Use SvelteKit-style procedure type annotations.

```rust
#[remote(query)]
#[remote(command)]
#[remote(form)]
```

**Alternatives considered:**

- A: `#[remote(query)]` / `#[remote(command)]` / `#[remote(form)]` — explicit, mirrors SvelteKit
- B: `#[remote(GET)]` / `#[remote(POST)]` — HTTP methods, loses semantic meaning
- C: `#[remote]` with inference — too ambiguous, can't reliably infer intent from signature

**Rationale:** Explicit is better than implicit. SvelteKit uses query/command/form and developers familiar with SvelteKit will immediately understand the semantics. `form` specifically means "progressive enhancement form submission" — this distinction matters for DX.

---

## 2. Client Boundary: SvelteKit Only

**Decision:** The browser never calls Rust directly. All calls go through SvelteKit remote functions.

**Rationale:**

- Security: `.remote.ts` files are server-only by SvelteKit convention. No risk of leaking server code to client.
- Flexibility: SvelteKit BFF can validate (Zod), transform, cache, and handle auth before calling Rust.
- Colocation: Data loading is next to the component that uses it, not scattered across `+page.server.ts` files.
- Progressive enhancement: form procedures work without JS via SvelteKit's built-in enhancement.

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

**Decision:** SvelteKit BFF forwards cookies to Rust by default. Optional `Authorization` header override for specific calls.

**Rationale:** The default case (same-origin BFF → Rust) should "just work" with cookies. The explicit override exists for API keys, service-to-service calls, and cross-origin scenarios. This matches the SvelteKit remote function model where cookies are available via `getRequestEvent()`.

---

## 9. Serialization: JSON Only

**Decision:** All data between SvelteKit and Rust is JSON. No binary serialization.

**Rationale:**

- The hot path is Browser → SvelteKit, which is always JSON anyway.
- SvelteKit → Rust is localhost, where JSON is sub-millisecond.
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

**Decision:** SvelteKit remote functions validate with Zod (UX). Rust validates with serde + business logic (security).

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
