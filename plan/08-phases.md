# teleport-rs — Implementation Phases

## Phase 1: Core Types + Basic Generation

**Goal:** Define a Rust procedure, generate a TypeScript type file. No HTTP yet.

**Duration:** 3-5 days

### Tasks

- [x] Create workspace with `teleport`, `teleport-macros`, `teleport-build` crates
- [x] Implement `AppError<T>` with `Serialize`, `Deserialize`, `specta::Type`
- [ ] Implement `TeleportType` derive macro (wraps `Serialize + Deserialize + specta::Type`)
- [x] Implement `ProcedureRegistration` struct for `inventory`
- [ ] Implement `#[remote(query)]`, `#[remote(command)]`, `#[remote(form)]` proc macro
  - [ ] Parse function signature (ctx type, input type, output type, error type)
  - [ ] Validate return type is `Result<Output, AppError<E>>`
  - [ ] Generate `inventory::submit!` block with procedure metadata
  - [ ] Calculate namespace from module path
  - [ ] Support `name = "..."` and `prefix = "..."` overrides
- [ ] Implement `teleport-build::generate()` that:
  - [ ] Collects all procedures via `inventory::collect`
  - [ ] Generates `types.ts` from Specta type info
  - [ ] Generates `errors.ts` with `AppError<T>`, `TransportError`, `RpcResult`, procedure-specific aliases
  - [ ] Writes files to configured output directory
- [ ] Implement `src/bin/export.rs` binary that calls `teleport_build::generate()`
  - [ ] Reads `TELEPORT_OUTPUT_DIR` env var with fallback to default path
- [ ] Write unit tests for type generation
- [ ] Write integration test: define procedures, run `cargo run --bin export`, verify output matches expected

### Deliverable

A Rust crate where `#[remote(query)] fn get_user(...)` proc macro compiles, and running `cargo run --bin export` produces valid TypeScript type definitions.

---

## Phase 2: Router + Axum Integration

**Goal:** Generated procedures serve as actual HTTP endpoints via Axum.

**Duration:** 3-5 days

### Tasks

- [ ] Implement `TeleportRouter` struct
  - [ ] `new()` — creates empty router
  - [ ] `state(state: Arc<AppState>)` — sets shared state
  - [ ] `mount()` — collects procedures and builds Axum Router
- [ ] Implement Axum route generation from `ProcedureRegistration`
  - [ ] GET routes for `query` procedures (input as query params)
  - [ ] POST routes for `command` procedures (input as JSON body)
  - [ ] POST routes for `form` procedures (input as JSON body, from FormData)
  - [ ] All routes under `/rpc/{namespace}.{name}` prefix
- [ ] Implement `AppError<T>` → Axum `IntoResponse`
  - [ ] Map error variants to HTTP status codes
  - [ ] Serialize error body as JSON
- [ ] Implement `TeleportType` → Axum extractor bridge
  - [ ] Deserialize input from query params (GET) or JSON body (POST)
  - [ ] Inject `State<Arc<AppState>>` as first parameter
  - [ ] Inject `Extension<AuthedUser>` if present in signature
- [ ] Add optional debug manifest endpoint `GET /rpc/__manifest`
- [ ] Write integration tests with `axum::test`
  - [ ] Test GET query with input
  - [ ] Test POST command with input
  - [ ] Test error responses (AppError variants)
  - [ ] Test 404 for unknown procedures

### Deliverable

A working Axum server that serves `#[remote]` procedures as HTTP endpoints. You can `curl /rpc/users.getUser?id=123` and get a typed JSON response.

---

## Phase 3: TypeScript Client Generator

**Goal:** Auto-generate the `client.ts` file with typed RPC functions.

**Duration:** 3-5 days

### Tasks

- [ ] Implement `@teleport-rs/client` npm package
  - [ ] `rpc()` function — core HTTP fetch wrapper
  - [ ] Result type — `RpcResult<T, E>` with transport vs app error distinction
  - [ ] Helper functions — `isAppError()`, `isTransportError()`, `unwrap()`
  - [ ] `configure()` — set baseUrl, timeout, credentials, headers
- [ ] Implement client generation in `teleport-build`
  - [ ] Generate namespace objects (`auth`, `users`, `posts`)
  - [ ] Generate `rpc()` calls with correct types for each procedure
  - [ ] snake_case → camelCase naming conversion
  - [ ] Handle procedures with no input (void input)
  - [ ] Handle procedures with `Option<AuthedUser>` (auth-required vs auth-optional)
- [ ] Write `client.ts` template with proper imports from `types.ts` and `errors.ts`
- [ ] Write unit tests for naming conversion
- [ ] Write integration tests: define Rust procedures, generate client.ts, verify it compiles with TS

### Deliverable

Running `teleport-build::generate()` produces `types.ts`, `errors.ts`, and `client.ts`. The client can be imported in a TypeScript project and has full type safety.

---

## Phase 4: Auth Middleware

**Goal:** Cookie forwarding, session extraction, `AuthedUser` parameter in procedures.

**Duration:** 2-3 days

### Tasks

- [ ] Implement auth middleware in Axum
  - [ ] Extract `session_id` from cookies
  - [ ] Extract `Authorization: Bearer <token>` header
  - [ ] Validate session via `AppState.db` or `AppState.redis`
  - [ ] Store `AuthedUser` in request extensions
- [ ] Implement `AuthedUser` as Axum extractor
  - [ ] Returns 401 if not present (required auth)
  - [ ] `Option<AuthedUser>` returns `None` if not present (optional auth)
- [ ] Update proc macro to support `AuthedUser` parameter
  - [ ] Parse function signatures with `auth: AuthedUser` or `auth: Option<AuthedUser>`
  - [ ] Generate extractor code in Axum handler
- [ ] Update client to forward cookies from SvelteKit `getRequestEvent()`
- [ ] Write integration tests:
  - [ ] Unauthenticated request to auth-required procedure → 401
  - [ ] Authenticated request to auth-required procedure → success
  - [ ] Optional auth returns `None` when not authenticated

### Deliverable

Procedures can declare `auth: AuthedUser` in their signature and the framework automatically extracts session, validates it, and injects it. SvelteKit remote functions forward cookies seamlessly.

---

## Phase 5: Vite Plugin + Dev Experience

**Goal:** Auto-regeneration on Rust changes, HMR in SvelteKit.

**Duration:** 2-3 days

### Tasks

- [ ] Implement `@teleport-rs/vite` plugin
  - [ ] Watch for changes in generated/ directory
  - [ ] Trigger full reload on binding changes
  - [ ] Optional: trigger `cargo build` on Rust file changes
- [ ] Verify `src/bin/export.rs` integrates with dev workflow
  - [ ] `cargo watch -x 'run --bin export'` triggers TS regeneration on Rust changes
  - [ ] Only write files if content changed (avoid unnecessary HMR)
- [ ] Write SvelteKit integration example:
  - [ ] `src/lib/api/config.ts` — configure rpc client
  - [ ] `src/lib/api/index.ts` — barrel exports
  - [ ] `src/lib/server/data.remote.ts` — example remote functions using generated client
- [ ] Write dev setup guide (cargo-watch + SvelteKit dev server)
- [ ] Test the full dev loop:
  - [ ] Change Rust procedure → cargo build → TS regenerates → SvelteKit HMR
  - [ ] Verify TypeScript errors show up in IDE
  - [ ] Verify no-stale-binding issues

### Deliverable

A smooth dev experience where changing a Rust procedure automatically updates the TypeScript client, and SvelteKit hot-reloads with the new types.

---

## Phase 6: SvelteKit Remote Functions Integration Guide

**Goal:** Documented patterns for using teleport-rs with SvelteKit remote functions.

**Duration:** 2-3 days

### Tasks

- [ ] Write comprehensive example `data.remote.ts` files:
  - [ ] Query pattern (getUser)
  - [ ] Command pattern (login, logout)
  - [ ] Form pattern (createPost)
  - [ ] Error handling patterns (transport vs app errors)
  - [ ] Auth patterns (login/set cookie, getMyProfile)
- [ ] Write SvelteKit hook for cookie handling (`hooks.server.ts`)
- [ ] Write example pages using remote functions:
  - [ ] Login page with form validation
  - [ ] Profile page with authenticated data
  - [ ] Posts page with CRUD operations
- [ ] Write example Rust server with full auth flow
- [ ] Test with SvelteKit remote functions (require SvelteKit experimental flag)

### Deliverable

A complete, working example app that demonstrates the full integration between teleport-rs and SvelteKit remote functions, with auth, error handling, and all three procedure types.

---

## Phase 7: Polish + Validation Bridge (Future)

**Goal:** Specta → Zod bridge, better DX, production readiness.

**Duration:** Ongoing

### Tasks

- [ ] Investigate `specta-zod` for auto-generating Zod schemas from Rust types
- [ ] Add router merging (split procedures across files)
- [ ] Add SSE/streaming support for real-time use cases
- [ ] Performance benchmarks (latency, throughput)
- [ ] Security audit (input validation, CSRF, rate limiting)
- [ ] Add request logging/middleware hooks
- [ ] Explore binary serialization opt-in
- [ ] Write comprehensive documentation
- [ ] Publish crates.io + npm

---

## Summary Timeline

| Phase | Duration | Key Milestone                       |
| ----- | -------- | ----------------------------------- |
| 1     | 3-5 days | Proc macro + TS type generation     |
| 2     | 3-5 days | Axum router serves HTTP endpoints   |
| 3     | 3-5 days | Generated TS client with full types |
| 4     | 2-3 days | Auth middleware + cookie forwarding |
| 5     | 2-3 days | Vite plugin + dev loop              |
| 6     | 2-3 days | SvelteKit integration examples      |
| 7     | Ongoing  | Polish, docs, validation bridge     |

**Total estimated time for phases 1-6: 15-24 days** for a solo developer working full-time.
