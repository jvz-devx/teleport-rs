---
name: teleport-rust
description: >
  Rust coding skill for the teleport-rs project. MUST be used whenever writing, editing, or reviewing
  any Rust code (.rs files) in this workspace — including proc macros, the export binary, library code,
  tests, and Cargo.toml changes. Triggers on: Rust implementation, adding procedures, error types,
  Axum handlers, Specta types, inventory registration, macro expansion, Cargo dependency changes,
  or any discussion about how to write Rust code in this project.
---

# teleport-rs Rust Coding Skill

You are working on **teleport-rs**, a framework that bridges Rust (Axum) backend procedures to a
TypeScript (SvelteKit) frontend via auto-generated typed clients. The project uses proc macros,
Specta for type introspection, and the inventory crate for runtime procedure collection.

## Project Architecture

### Workspace Crates

| Crate | Purpose | Key exports |
|---|---|---|
| `teleport` | Core library — router, error types, extractors, re-exports macro | `TeleportRouter`, `AppError<T>`, `#[remote]`, `TeleportType` |
| `teleport-macros` | Proc macro crate — `#[remote]` attribute and `TeleportType` derive | `remote`, `TeleportType` |
| `teleport-build` | TS generation engine — reads procedure registry, writes .ts files | `generate(Config)` |

### Key Dependencies

- **axum 0.8** — HTTP framework. Use its extractor pattern idiomatically.
- **specta 2** + **specta-typescript 0.0.8** — Type introspection to TypeScript. Every type crossing the boundary needs `specta::Type`.
- **inventory 0.3** — Runtime procedure collection via `inventory::collect!` / `inventory::submit!`. This is runtime-only — it does NOT work in build scripts.
- **serde 1** — Serialization. All boundary types need `Serialize + Deserialize`.
- **tokio** — Async runtime, full features.

### How It Fits Together

1. `#[remote(query|command|form)]` proc macro generates an Axum handler + `inventory::submit!` block
2. `TeleportRouter::mount()` calls `inventory::collect` at runtime to discover all procedures
3. `cargo run --bin export` runs the same collection and writes `types.ts`, `errors.ts`, `client.ts`
4. The SvelteKit frontend imports the generated client — browser never calls Rust directly

## Error Handling

This is the most important section. The project uses `AppError<T>` as its error type.

### Rules

- **Never use `.unwrap()`**. Not in library code, not in binaries, not anywhere in production paths.
- **Never use `.expect()`** in library code. In `main.rs`, `export.rs`, and test code, `.expect("descriptive reason")` is acceptable — but only when the failure genuinely means "the program cannot continue and here's why".
- **Use `?` for propagation.** Convert errors at boundaries with `.map_err()` or implement `From`.
- **Return `Result` from everything that can fail.** Don't panic as flow control.

### The AppError Pattern

```rust
// AppError<T> is the framework's error type. T is the procedure-specific detail.
// Most procedures use a concrete error detail type:
async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError<GetUserError>> { ... }

// Procedures without specific errors use the default:
async fn health_check(ctx: &AppState) -> Result<(), AppError> { ... }
```

When adding new error variants, put shared errors in `AppError` (Unauthorized, NotFound, Internal, etc.)
and procedure-specific errors in a dedicated detail struct. Don't let `AppError` become a god enum.

### Error Conversion

Prefer `From` impls over scattered `.map_err()` when converting from external error types:

```rust
// Good — implement once, use ? everywhere
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

// Avoid — repetitive and noisy
ctx.db.get_user(&id).await.map_err(|e| AppError::Internal(e.to_string()))?;
```

## Code Style

### Edition and Toolchain

- **Rust 2024 edition** (MSRV 1.91). Use edition 2024 features where they improve clarity.
- **Resolver 3** in workspace Cargo.toml.

### Clippy

The project uses a strict clippy configuration. Write code that passes `clippy::pedantic` without
suppression attributes. When a pedantic lint genuinely doesn't apply, use a scoped
`#[allow(clippy::specific_lint)]` with a comment explaining why — never blanket `#[allow(clippy::all)]`.

### Cloning and Ownership

- **Don't clone to satisfy the borrow checker.** If you're reaching for `.clone()`, step back and
  think about whether a reference, a lifetime, or restructuring would work.
- **Arc usage:** Clone `Arc` once when passing to a new owner (spawning a task, moving into a closure).
  Don't `.clone()` an `Arc` on every function call — pass `&Arc<T>` or `&T` instead.
- **String ownership:** Accept `&str` in function parameters unless you need ownership. Return `String`
  when the caller will own it. Don't accept `String` just to immediately borrow it.

```rust
// Good — borrows what it reads, owns what it must
fn find_user(db: &PgPool, id: &str) -> Result<User, AppError> { ... }

// Bad — takes ownership for no reason
fn find_user(db: PgPool, id: String) -> Result<User, AppError> { ... }
```

### Unsafe

Unsafe code requires strong justification. The project's domain (HTTP framework, type generation)
has almost no reason for unsafe. If you think you need it, you probably don't — look for a safe
abstraction first. If unsafe is genuinely required (e.g., FFI, performance-critical path with
measured benchmarks), isolate it in a minimal block with a `// SAFETY:` comment explaining the
invariants.

### Derive Order

Use consistent derive ordering across the project:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct User { ... }

// Or with the convenience derive:
#[derive(Debug, Clone, TeleportType)]
pub struct User { ... }
```

### Module Organization

- One domain module per file under `src/api/` (e.g., `users.rs`, `auth.rs`, `posts.rs`)
- Module path becomes the procedure namespace: `src/api/users.rs` -> `/rpc/users.getUser`
- Keep types close to where they're used. Shared types go in a `types.rs` module.
- Don't create `utils.rs` or `helpers.rs` grab-bags.

## Proc Macro Conventions (teleport-macros)

When working on the proc macro crate:

- Use `syn 2` for parsing, `quote` for code generation, `proc-macro2` for token manipulation.
- Return compile errors via `syn::Error` → `TokenStream`, not panics.
- Test macro expansion with `cargo expand` or snapshot tests, not by eyeballing generated code.
- The macro should validate at compile time: correct signature, required trait bounds, valid
  procedure type. Give clear error messages pointing at the offending span.

## Writing Procedures

### Signature

```rust
#[remote(query)]
pub async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError<GetUserError>> {
    // ...
}
```

- First param: `&AppState` (always)
- Optional: `AuthedUser` or `Option<AuthedUser>` for auth-required/optional procedures
- Input param: the deserialized input type (must impl `Serialize + Deserialize + specta::Type`)
- Return: `Result<T, AppError<E>>` where T and E both impl `Serialize + specta::Type`

### Choosing Procedure Type

| Use `query` | Use `command` | Use `form` |
|---|---|---|
| Read-only, no side effects | Mutations, writes, actions | Form submissions |
| GET with query params | POST with JSON body | POST with progressive enhancement |
| Cacheable | Not cacheable | Works without JS |

Don't use `command` for reads just because the input is complex — that's what `qs`-style query
serialization is for. Use `command` only when the operation has side effects.

## Testing

- Use `#[tokio::test]` for async tests.
- Integration tests for router/handler behavior go in `tests/` using `axum::test`.
- Unit tests for pure logic go in `#[cfg(test)] mod tests` within the source file.
- `.expect("reason")` is fine in tests for setup/teardown that shouldn't fail.
- Test the error paths, not just the happy path. Verify `AppError` variants serialize correctly.

```rust
#[tokio::test]
async fn get_user_returns_not_found_for_missing_id() {
    let app = test_app().await;
    let response = app.get("/rpc/users.getUser?id=nonexistent").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: AppError<GetUserError> = response.json().await;
    assert!(matches!(body, AppError::NotFound));
}
```

## Specta and Type Generation

- Every type that crosses the Rust-TS boundary must derive `specta::Type` (or `TeleportType`).
- `specta-typescript` version must match Specta v2's API — check compatibility on upgrade.
- When adding a new type, think about how it looks in TypeScript. Rust enums with data become
  TypeScript discriminated unions. Keep variants simple and serialization-friendly.
- Don't use `#[serde(flatten)]` unless you've verified Specta handles it correctly for TS output.

## Cargo.toml Conventions

- All shared dependencies go in `[workspace.dependencies]` with version pinned there.
- Crate-level Cargo.toml references workspace deps: `axum = { workspace = true }`.
- Use `edition.workspace = true` and `rust-version.workspace = true` in member crates.
- Don't add dependencies without considering whether they belong at workspace level.
