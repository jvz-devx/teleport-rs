# teleport-rs — DX Audit Report

Structured audit of developer experience issues found after Phase 7a implementation. Items are categorized by severity and type.

---

## Bugs (fix immediately)

### B1: Double /rpc prefix

**Symptom:** Requests go to `/rpc/rpc/auth.login` instead of `/rpc/auth.login`.

**Cause:** Default `baseUrl` in `RpcConfig` is `"/rpc"`, and generated paths already include the `/rpc` prefix (e.g., `"/rpc/auth.login"`). The `rpc()` function concatenates them: `baseUrl + path`.

**Fix:** Change default `baseUrl` to `""` (empty string). Generated paths keep the `/rpc` prefix. Users set `baseUrl` to their server origin (e.g., `"http://localhost:3000"`).

### B2: void return crash

**Symptom:** `rpc()` throws `SyntaxError: Unexpected end of JSON input` when a procedure returns `()`.

**Cause:** `rpc()` always calls `response.json()` on success, but Rust procedures returning `()` produce an empty response body (or 204 No Content).

**Fix:** Check for 204 status or empty `Content-Length` before calling `.json()`. Return `undefined` for void responses.

### B3: Serialization error swallowed

**Symptom:** Procedure returns empty body with 200 status when serialization fails.

**Cause:** `AppError::into_response()` uses `serde_json::to_string(&self).unwrap_or_default()`, which silently returns `""` if the error type itself fails to serialize.

**Fix:** Replace `unwrap_or_default()` with a fallback that returns a generic 500 JSON error body (e.g., `{"code":"internal","message":"serialization failed"}`).

---

## Silent failures

### S1: Zero procedures — empty TS with no warning

**Symptom:** `cargo run --bin export` produces valid but empty TypeScript files. No procedures appear in the generated client.

**Cause:** Rust modules containing `#[remote]` procedures must be imported via `use` statements in the export binary's crate graph. If the `mod` / `use` wiring is missing, `inventory::collect` finds nothing.

**Mitigation:** Emit a warning to stderr when zero procedures are collected: `"warning: no procedures found. Did you forget to import your procedure modules?"`.

### S2: Missing .state() — empty router

**Symptom:** Server starts successfully but serves no routes. All requests return 404.

**Cause:** `TeleportRouter::mount()` returns an empty `axum::Router` when `.state()` was never called. No error or warning is emitted.

**Mitigation:** Log a warning or return an error from `mount()` when state is not set but procedures expect it.

### S3: State type mismatch — procedures silently skipped

**Symptom:** Some procedures don't appear in the mounted router. No error at startup.

**Cause:** If a procedure's state type doesn't match the type passed to `.state()`, it's silently excluded during route registration.

**Mitigation:** Panic at `mount()` time with a clear message listing which procedures were skipped and why.

### S4: Missing pub mod — procedure absent from export

**Symptom:** New procedure file exists but doesn't appear in generated TypeScript.

**Cause:** Rust requires explicit `pub mod` declarations to include a file in the module tree. Without it, the module is never compiled and `inventory` never sees its procedures.

**Mitigation:** This is fundamental to Rust's module system and can't be auto-detected. Document prominently in getting-started guide. Consider a `cargo teleport check` command that compares `.rs` files in the procedures directory against the module tree.

### S5: Zod schema drift

**Symptom:** Rust type changes don't cause TypeScript compilation errors because Zod schemas are hand-written.

**Cause:** Decision 14 deferred Zod auto-generation. Hand-written Zod schemas in SvelteKit remote functions can drift from generated TypeScript types.

**Mitigation:** Document the risk. Long-term: revisit `specta-zod` or generate Zod schemas alongside TypeScript types.

---

## API ergonomics

### A1: Config has no Default

**Problem:** `RpcConfig` requires 6 mandatory fields, but 4 have obvious defaults (`baseUrl: ""`, `timeout: 30000`, `credentials: "include"`, `headers: () => ({})`).

**Fix:** Implement `Default` / default values so only non-obvious fields are required.

### A2: AuthedUser is a fixed struct

**Problem:** `AuthedUser { id: String, email: String }` is hardcoded. Real apps need roles, permissions, tenant IDs, etc.

**Fix:** Make the router generic over the user type `U`. Auth middleware validator returns `Option<U>`. The `#[remote]` macro detects auth parameters by trait bound (e.g., `Clone + Send + Sync + 'static`), not by type name. See Decision 24.

### A3: RpcResult three-branch union

**Problem:** Checking `"transport" in result` is not idiomatic TypeScript. The three-branch union (`ok`, `error`, `transport`) requires awkward narrowing.

**Fix:** Consider a two-branch union where transport errors are wrapped in the error branch with a `kind` discriminant. Or provide helper functions that make the narrowing ergonomic (partially addressed by `isAppError()` / `isTransportError()`).

### A4: No global error handler / interceptor

**Problem:** No way to handle 401 globally (e.g., redirect to login page). Every call site must check for unauthorized errors individually.

**Fix:** Add `onError` callback to `RpcConfig`, called on every failed RPC. Keep `RpcResult` return (don't swallow errors), just notify. See Decision 25.

### A5: Content-Type: application/json on GET requests

**Problem:** GET requests (query procedures) send `Content-Type: application/json` header even though GET requests have no body.

**Fix:** Only set `Content-Type` header on POST requests.

### A6: Generated namespace objects not tree-shakeable

**Problem:** Generated namespace objects (`auth`, `users`) bundle all procedures. Bundlers can't tree-shake unused procedures because object property access is opaque.

**Fix:** Consider generating individual exported functions (e.g., `auth_login()`) alongside namespace objects, or use a pattern that bundlers can analyze.

### A7: Generated handler visibility leaks

**Problem:** `__teleport_handler_*` functions generated by the proc macro are visible in the public API of procedure modules.

**Fix:** Generate handlers inside a private `mod __teleport_internal { }` or use `#[doc(hidden)]` + naming convention that won't collide.

---

## Missing features

### M1: No scaffolding tool

No `cargo teleport init` or `npx create-teleport` to bootstrap a new project. Users must manually create workspace, crates, config, and export binary.

### M2: No setup documentation

No getting-started guide, README, or tutorial. Users must read the example app and reverse-engineer the setup.

### M3: No TanStack Query / React Query adapter

Decision 23 repositioned teleport-rs as framework-agnostic, but no React/TanStack Query integration exists yet.

### M4: No request/response interceptors

Beyond the `headers()` config option, there's no way to intercept requests (add auth tokens, logging) or responses (transform data, retry logic).

### M5: No request tracing / correlation IDs

No built-in support for propagating trace IDs or correlation IDs through the RPC chain for debugging distributed calls.

### M6: Stale bindings detection in Vite plugin

The Vite plugin watches for file changes but doesn't detect when generated bindings are stale relative to the Rust source (e.g., when the export binary hasn't been re-run).

### M7: Auto-generate barrel index.ts

Generated output doesn't include an `index.ts` barrel file that re-exports from `types.ts`, `client.ts`, and `errors.ts`.

---

## Priority ranking — Top 5 fixes

| Priority | Item | Reason |
|----------|------|--------|
| 1 | B1: Double /rpc prefix | Every user hits this immediately — app is broken out of the box |
| 2 | B2: void return crash | Common pattern (logout, delete) crashes at runtime |
| 3 | A2: AuthedUser generic | Blocks any real app beyond demos |
| 4 | B3: Serialization swallowed | Silent data loss in production |
| 5 | A4: Global error handler | Required for any app with authentication |
