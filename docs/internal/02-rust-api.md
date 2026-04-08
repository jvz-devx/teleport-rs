# teleport-rs — Rust API Design

## Proc Macro: `#[remote]`

### Syntax

```rust
use teleport_rs::{remote, AppError, TeleportRouter};
use serde::{Serialize, Deserialize};

// Option 1: Minimal — just the procedure type
#[remote(query)]
async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError<GetUserError>> {
    ctx.db.get_user(&id).await.map_err(|e| AppError::Internal(e.to_string()))
}

// Option 2: With name override
#[remote(query, name = "fetchProfile")]
async fn get_user_profile(ctx: &AppState, id: String) -> Result<Profile, AppError<GetProfileError>> {
    ctx.db.get_profile(&id).await.map_err(|e| AppError::Internal(e.to_string()))
}

// Option 3: With prefix override (overrides module namespace)
#[remote(command, prefix = "admin")]
async fn delete_user(ctx: &AppState, id: String) -> Result<(), AppError<DeleteUserError>> {
    ctx.db.delete_user(&id).await.map_err(|e| AppError::Internal(e.to_string()))
}

// Option 4: Form procedure
#[remote(form)]
async fn create_post(ctx: &AppState, form: CreatePostForm) -> Result<Post, AppError<CreatePostError>> {
    ctx.db.create_post(form.into()).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

### Procedure Types

| Type      | HTTP Method | Input Location | Use Case                                                                                                    |
| --------- | ----------- | -------------- | ----------------------------------------------------------------------------------------------------------- |
| `query`   | GET         | Query params   | Read-only data fetching                                                                                     |
| `command` | POST        | JSON body      | Mutations, actions                                                                                          |
| `form`    | POST        | Form-urlencoded or JSON | Form submissions with progressive enhancement. Uses the `FormOrJson` extractor: HTML forms submit url-encoded data natively, while JS clients can send JSON. |

### Function Signature Rules

All `#[remote]` functions must follow this signature:

```rust
// query: (context, input) -> Result<Output, AppError<E>>
#[remote(query)]
async fn name(ctx: &AppState, input: InputType) -> Result<OutputType, AppError<ErrorType>> {}

// query with no input: (context) -> Result<Output, AppError<E>>
#[remote(query)]
async fn name(ctx: &AppState) -> Result<OutputType, AppError<ErrorType>> {}

// command: same as query but POST
#[remote(command)]
async fn name(ctx: &AppState, input: InputType) -> Result<OutputType, AppError<ErrorType>> {}

// form: accepts both form-urlencoded and JSON via FormOrJson extractor
#[remote(form)]
async fn name(ctx: &AppState, form: FormType) -> Result<OutputType, AppError<ErrorType>> {}
```

**Compile-time checks enforced by the macro:**

- First parameter must be a reference to the state type
- Return type must be `Result<T, AppError<E>>`
- Function must be async
- Input type must implement `Serialize + Deserialize + Type` (Specta)
- Output type must implement `Serialize + Type`
- Error type must implement `Serialize + Type`

### Context (AppState)

The state type is defined by the user and passed to `TeleportRouter::new()`:

```rust
use std::sync::Arc;
use sqlx::PgPool;

pub struct AppState {
    pub db: PgPool,
    pub redis: RedisPool,
    pub config: AppConfig,
}

// In main.rs
let state = Arc::new(AppState { db, redis, config });

let app = Router::new()
    .merge(
        TeleportRouter::new()
            .state(state)
            .mount()
    );
```

The state is injected via Axum's `State` extractor. The `#[remote]` macro generates the Axum handler that extracts `State<Arc<AppState>>` and passes it to your function.

### Auth: AuthedUser as Explicit Parameter

For procedures that require authentication, `AuthedUser` is an explicit function parameter extracted from Axum request extensions. Auth middleware stores the user in extensions, and the `#[remote]` macro generates an extractor that pulls it out.

```rust
// lib/extractors.rs
use axum::extract::FromRequestParts;

pub struct AuthedUser {
    pub id: String,
    pub email: String,
}

impl FromRequestParts<AppState> for AuthedUser {
    // Extract from request extensions (set by auth middleware)
    // Returns 401 if not authenticated
}

// Usage in remote procedure — AuthedUser as explicit parameter
#[remote(query)]
async fn get_my_orders(ctx: &AppState, auth: AuthedUser) -> Result<Vec<Order>, AppError<GetOrdersError>> {
    ctx.db.get_orders(&auth.id).await.map_err(|e| AppError::Internal(e.to_string()))
}

// Optional auth — use Option<AuthedUser>
#[remote(query)]
async fn get_public_profile(ctx: &AppState, auth: Option<AuthedUser>, id: String) -> Result<Profile, AppError<GetProfileError>> {
    let profile = ctx.db.get_profile(&id).await.map_err(|e| AppError::Internal(e.to_string()))?;
    // auth.is_some() → show private fields
    Ok(profile)
}
```

The `#[remote]` macro detects auth parameters in the parameter list — either `AuthedUser`/`Option<AuthedUser>` by convention, or any type annotated with `#[auth]`/`Option<T>` with `#[auth]` — and generates the appropriate Axum extraction. `AppState` stays clean — no per-request mutation needed. See `05-auth.md` for details.

## Router Generation

The `#[remote]` macro generates an Axum route for each procedure. At runtime, `TeleportRouter::mount()` collects all registered procedures via `inventory::collect` and produces:

1. An Axum `Router` with all `/rpc/*` routes
2. A route manifest (for debugging)

TypeScript generation is handled separately by running `cargo run` with export support (see `06-build-pipeline.md`).

### Registration Mechanism

Using inventory pattern (like `linkme` or `ctor`):

```rust
// In teleport-macros/src/lib.rs
// #[remote] generates:
//
// ::teleport_rs::__private::register_procedure!(Procedure {
//     name: "users.getUser",
//     method: GET,
//     handler: get_user,
//     input_type: <String as Type>::inline(),
//     output_type: <User as Type>::inline(),
//     error_type: <GetUserError as Type>::inline(),
// });
```

At runtime, `TeleportRouter::new()` collects all registered procedures via `inventory::collect`.

### Route Manifest

Debug endpoint at `GET /rpc/__manifest` returns:

```json
{
  "procedures": {
    "users.getUser": { "method": "GET", "input": "String", "output": "User" },
    "users.getPosts": {
      "method": "GET",
      "input": "String",
      "output": "Vec<Post>"
    },
    "auth.login": {
      "method": "POST",
      "input": "LoginRequest",
      "output": "AuthToken"
    }
  }
}
```

This is optional and can be disabled in production for security.

## Derive Requirements

All types passed across the boundary need:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
```

teleport-rs provides a convenience derive:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TeleportType)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
```

Where `TeleportType` is a derive macro that expands to `Serialize + Deserialize + specta::Type`. This reduces boilerplate.

## Module Organization

Rust procedures are organized by domain module:

```rust
// src/api/mod.rs
pub mod users;
pub mod auth;
pub mod posts;
```

```rust
// src/api/users.rs
use teleport_rs::remote;
use crate::state::AppState;
use crate::types::*;
use crate::errors::*;

#[remote(query)]
pub async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError<GetUserError>> {
    ctx.db.get_user(&id).await.map_err(|e| AppError::Internal(e.to_string()))
}

#[remote(command)]
pub async fn update_user(ctx: &AppState, req: UpdateUserRequest) -> Result<User, AppError<UpdateUserError>> {
    ctx.db.update_user(req).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

The module path (`users`) becomes the namespace in the route: `/rpc/users.getUser`, `/rpc/users.updateUser`.

The proc macro resolves the namespace from the module path where the function is defined.
