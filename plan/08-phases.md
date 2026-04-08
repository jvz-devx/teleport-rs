# teleport-rs — Implementation Phases

## Phase 1: Core Types + Basic Generation

**Goal:** Define a Rust procedure, generate a TypeScript type file. No HTTP yet.

**Duration:** 3-5 days

### Tasks

- [x] Create workspace with `teleport`, `teleport-macros`, `teleport-build` crates
- [x] Implement `AppError<T>` with `Serialize`, `Deserialize`, `specta::Type`
- [x] Implement `teleport_type` attribute macro (prepends `Serialize + Deserialize + specta::Type` derives)
- [x] Implement `ProcedureRegistration` struct for `inventory`
- [x] Implement `#[remote(query)]`, `#[remote(command)]`, `#[remote(form)]` proc macro
  - [x] Parse function signature (ctx type, input type, output type, error type)
  - [x] Validate return type is `Result<Output, AppError<E>>`
  - [x] Generate `inventory::submit!` block with procedure metadata
  - [x] Calculate namespace from module path
  - [x] Support `name = "..."` and `prefix = "..."` overrides
- [x] Implement `teleport-build::generate()` that:
  - [x] Collects all procedures via `inventory::collect`
  - [x] Generates `types.ts` from Specta type info
  - [x] Generates `errors.ts` with `AppError<T>`, `TransportError`, `RpcResult`, procedure-specific aliases
  - [x] Writes files to configured output directory
- [x] Implement export binary pattern (integration test acts as export binary)
  - [x] Converts `ProcedureRegistration` to `ProcedureInfo` with shared `Types` collection
- [x] Write unit tests for type generation (20 tests in teleport-build)
- [x] Write integration test: define procedures, generate TS, verify output matches expected (5 tests)

### Deliverable

A Rust crate where `#[remote(query)] fn get_user(...)` proc macro compiles, and running `cargo run --bin export` produces valid TypeScript type definitions.

---

## Phase 2: Router + Axum Integration

**Goal:** Generated procedures serve as actual HTTP endpoints via Axum.

**Duration:** 3-5 days

### Tasks

- [x] Implement `TeleportRouter` struct
  - [x] `new()` — creates empty router
  - [x] `state(state: Arc<AppState>)` — sets shared state
  - [x] `mount()` — collects procedures and builds Axum Router
- [x] Implement Axum route generation from `ProcedureRegistration`
  - [x] GET routes for `query` procedures (input via serde_qs query params)
  - [x] POST routes for `command` procedures (input as JSON body)
  - [x] POST routes for `form` procedures (input as JSON body, from FormData)
  - [x] All routes under `/rpc/{namespace}.{name}` prefix
- [x] Implement `AppError<T>` → Axum `IntoResponse`
  - [x] Map error variants to HTTP status codes
  - [x] Serialize error body as JSON
- [x] Implement `TeleportType` → Axum extractor bridge
  - [x] Deserialize input from query params (GET) or JSON body (POST)
  - [x] Inject `State<Arc<AppState>>` as first parameter
  - [x] Inject `Extension<AuthedUser>` if present in signature
- [x] Add optional debug manifest endpoint `GET /rpc/__manifest`
- [x] Write integration tests with `axum::test`
  - [x] Test GET query with input
  - [x] Test POST command with input
  - [x] Test error responses (AppError variants)
  - [x] Test 404 for unknown procedures
  - [x] Test auth (with/without AuthedUser)
  - [x] Test form procedure

### Deliverable

A working Axum server that serves `#[remote]` procedures as HTTP endpoints. You can `curl /rpc/users.getUser?id=123` and get a typed JSON response.

---

## Phase 3: TypeScript Client Generator

**Goal:** Auto-generate the `client.ts` file with typed RPC functions.

**Duration:** 3-5 days

### Tasks

- [x] Implement `@teleport-rs/client` npm package
  - [x] `rpc()` function — core HTTP fetch wrapper (with qs serialization for GET)
  - [x] Result type — `RpcResult<T, E>` with transport vs app error distinction
  - [x] Helper functions — `isAppError()`, `isTransportError()`, `unwrap()`
  - [x] `configure()` — set baseUrl, timeout, credentials, headers
- [x] Implement client generation in `teleport-build`
  - [x] Generate namespace objects (`auth`, `users`, `posts`)
  - [x] Generate `rpc()` calls with correct types for each procedure
  - [x] snake_case → camelCase naming conversion
  - [x] Handle procedures with no input (void input)
  - [x] Handle procedures with `Option<AuthedUser>` (auth-required vs auth-optional)
- [x] Write `client.ts` template with proper imports from `types.ts` and `errors.ts`
- [x] Write unit tests for naming conversion
- [x] Write integration tests: define Rust procedures, generate client.ts, verify output

### Deliverable

Running `teleport-build::generate()` produces `types.ts`, `errors.ts`, and `client.ts`. The client can be imported in a TypeScript project and has full type safety.

---

## Phase 4: Auth Middleware

**Goal:** Cookie forwarding, session extraction, `AuthedUser` parameter in procedures.

**Duration:** 2-3 days

### Tasks

- [x] Implement auth middleware in Axum
  - [x] Extract `session_id` from cookies (configurable cookie name)
  - [x] Extract `Authorization: Bearer <token>` header (fallback)
  - [x] Validate session via closure-based validator with access to AppState
  - [x] Store `AuthedUser` in request extensions
- [x] Implement `AuthedUser` as Axum extractor
  - [x] Returns 401 if not present (required auth)
  - [x] `Option<AuthedUser>` returns `None` if not present (optional auth)
- [x] Update proc macro to support `AuthedUser` parameter
  - [x] Parse function signatures with `auth: AuthedUser` or `auth: Option<AuthedUser>`
  - [x] Generate extractor code in Axum handler
- [x] Client forwards cookies via `credentials: "include"` by default
- [x] Write integration tests:
  - [x] Unauthenticated request to auth-required procedure → 401
  - [x] Authenticated request to auth-required procedure → success
  - [x] Optional auth returns `None` when not authenticated
  - [x] Cookie-based auth via middleware
  - [x] Bearer token auth via middleware

### Deliverable

Procedures can declare `auth: AuthedUser` in their signature and the framework automatically extracts session, validates it, and injects it. SvelteKit remote functions forward cookies seamlessly.

---

## Phase 5: Vite Plugin + Dev Experience

**Goal:** Auto-regeneration on Rust changes, HMR in SvelteKit.

**Duration:** 2-3 days

### Tasks

- [x] Implement `@teleport-rs/vite` plugin
  - [x] Watch for changes in generated/ directory (granular HMR with module graph invalidation)
  - [x] Fallback to full reload if module graph resolution fails
  - [x] Optional: `generateOnStart` runs `cargo run --bin export` on dev server start
  - [x] Watcher cleanup on server close
- [x] Verify export binary integrates with dev workflow
  - [x] `cargo watch -x 'run --bin export'` triggers TS regeneration on Rust changes
  - [x] `write_if_changed()` in teleport-build skips unchanged files (avoids unnecessary HMR)
- [ ] Write SvelteKit integration example (Phase 6)
- [ ] Write dev setup guide (Phase 6)
- [ ] Test the full dev loop end-to-end (Phase 6)

### Deliverable

A smooth dev experience where changing a Rust procedure automatically updates the TypeScript client, and SvelteKit hot-reloads with the new types.

---

## Phase 6: SvelteKit Remote Functions Integration Guide

**Goal:** Documented patterns for using teleport-rs with SvelteKit remote functions.

**Duration:** 2-3 days

### Tasks

- [x] Write comprehensive example `data.remote.ts` files:
  - [x] Query pattern (getUser, listUsers)
  - [x] Command pattern (login, logout)
  - [x] Form pattern (createPost)
  - [x] Error handling patterns (transport vs app errors)
  - [x] Auth patterns (getMyProfile with AuthedUser)
- [x] Write SvelteKit layout with navigation and config import
- [x] Write example pages using remote functions:
  - [x] Login page with form validation and error handling
  - [x] Profile page with authenticated data
  - [x] Home page with user list
- [x] Write example Rust server with full auth flow (7 procedures, mock state)
- [x] Export binary generates all 3 TS files from example procedures

### Deliverable

A complete, working example app that demonstrates the full integration between teleport-rs and SvelteKit remote functions, with auth, error handling, and all three procedure types.

---

## Phase 7a: DX Refinement (Post-Implementation Fixes)

**Goal:** Fix the design issues identified during implementation. See `11-lessons-learned.md` for full context.

**Duration:** 2-3 days

### Tasks

- [ ] Eliminate `HttpMethod` duplication — have `teleport-build` depend on `teleport` directly, or create `teleport-core`
- [ ] Add `export_from_inventory(config)` convenience function in `teleport-build`
- [ ] Simplify `examples/demo/src/bin/export.rs` to use the one-liner
- [ ] Add `TeleportError` class to `@teleport-rs/client` (extends Error, carries AppError)
- [ ] Add `rpcUnwrap()` helper that throws `TeleportError` with full error detail
- [ ] Add `mapError()` combinator for transforming errors in the result pattern
- [ ] Simplify `examples/demo/frontend/src/lib/server/data.remote.ts` using new helpers
- [ ] Reposition docs/examples as framework-agnostic (not SvelteKit-only)

### Deliverable

The export binary drops from ~65 lines to ~5. The TS error handling boilerplate drops from 5-6 lines to 1. The project is positioned as a general Rust → TypeScript RPC framework.

---

## Phase 7b: Polish + Validation Bridge (Future)

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
| 1     | Done     | Proc macro + TS type generation     |
| 2     | Done     | Axum router serves HTTP endpoints   |
| 3     | Done     | Generated TS client with full types |
| 4     | Done     | Auth middleware + cookie forwarding |
| 5     | Done     | Vite plugin + dev loop              |
| 6     | Done     | Integration examples                |
| 7a    | Next     | DX refinement (lessons learned)     |
| 7b    | Ongoing  | Polish, docs, validation bridge     |

**Phases 1-6 completed.** Phase 7a addresses design issues discovered during implementation.
