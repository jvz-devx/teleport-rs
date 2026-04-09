# Getting Started

This guide walks you through building a simple API with teleport-rs.

## 1. Create a new Rust project

```bash
cargo new my-api
cd my-api
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
teleport = { version = "0.1", features = ["export"] }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
tower-http = { version = "0.6", features = ["cors"] }
```

## 2. Define your types

Create `src/types.rs`:

```rust
use teleport::teleport_type;

#[teleport_type]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
}
```

`#[teleport_type]` derives everything needed for serialization and TypeScript generation.

## 3. Write procedures

Create `src/api/users.rs`:

```rust
use teleport::{remote, AppError};
use crate::types::User;

#[remote(query)]
async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError> {
    // Your logic here — query a database, call a service, etc.
    ctx.get_user(&id).cloned().ok_or(AppError::NotFound)
}

#[remote(query)]
async fn list_users(ctx: &AppState) -> Result<Vec<User>, AppError> {
    Ok(ctx.list_users().to_vec())
}
```

Procedure types:

| Annotation | HTTP Method | Input Source | Use Case |
|---|---|---|---|
| `#[remote(query)]` | GET | Query params | Read-only fetches |
| `#[remote(command)]` | POST | JSON body | Mutations |
| `#[remote(form)]` | POST | Form-urlencoded or JSON | Progressive enhancement |

## 4. Set up the router

In `src/main.rs`:

```rust
use std::sync::Arc;
use teleport::{ExportConfig, TeleportRouter};

mod api;
mod types;

#[tokio::main]
async fn main() {
    // Export TypeScript bindings
    TeleportRouter::<AppState>::export(
        &ExportConfig::new("frontend/src/lib/api/generated"),
    ).expect("failed to export TS bindings");

    let state = Arc::new(AppState::new());

    let app = TeleportRouter::new()
        .state(Arc::clone(&state))
        .manifest(true)
        .mount();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server crashed");
}
```

`TeleportRouter::export()` writes three files to the output directory:
- `types.ts` — all your Rust types as TypeScript interfaces
- `client.ts` — typed RPC functions grouped by module
- `errors.ts` — error types matching your `AppError<T>` variants

### Safety defaults

`TeleportRouter::mount()` applies two safety layers to every router it
builds:

1. A **2 MiB request body size limit**
   (`tower_http::limit::RequestBodyLimitLayer`). Requests with larger
   bodies are rejected with `413 Payload Too Large` before any handler
   runs. Override with `.body_limit(bytes)` (for example, an upload
   procedure that needs 10 MiB) or disable entirely with
   `.no_body_limit()`.
2. **Panic recovery** via `tower_http::catch_panic::CatchPanicLayer`.
   A panic in any handler returns a generic JSON `500` instead of
   crashing the process. Opt out with `.no_catch_panic()` if you want
   panics to propagate (for example, under a supervisor that restarts
   on every crash).

```rust,ignore
let app = TeleportRouter::new()
    .state(state)
    .body_limit(10 * 1024 * 1024)  // 10 MiB
    .mount();

// or, if you fully trust every client of this router:
let app = TeleportRouter::new()
    .state(state)
    .no_body_limit()
    .no_catch_panic()
    .mount();
```

See [`security.md`](security.md) for the full production checklist.

## 5. Set up the frontend

Install the packages (bun, pnpm, or npm — all work):

```bash
bun add @teleport-rs/client @teleport-rs/vite
```

Configure the Vite plugin in `vite.config.ts`:

```typescript
import { teleportVite } from "@teleport-rs/vite";

export default {
    plugins: [
        teleportVite({
            bindingsPath: "src/lib/api/generated",
        }),
    ],
};
```

Use the generated client:

```typescript
import { users } from "./generated/client";

const result = await users.getUser("123");
if (result.ok) {
    console.log(result.data.name);
} else if ("error" in result) {
    // Application error from Rust
    console.error(result.error);
} else {
    // Transport error (network, timeout, etc.)
    console.error(result.transport);
}
```

## 6. Adding authentication

Set up cookie-based auth on the router. The auth validator returns `Option<U>` for any custom user type:

```rust
#[derive(Debug, Clone)]
struct MyUser {
    id: String,
    role: String,
}

let app = TeleportRouter::new()
    .state(Arc::clone(&state))
    .auth("session", |token: String, state: Arc<AppState>| async move {
        state.validate_session(&token)  // returns Option<MyUser>
    })
    .mount();
```

Then use your user type with `#[auth]` in procedure signatures:

```rust
use teleport::{remote, AppError};

#[remote(query)]
async fn get_profile(ctx: &AppState, #[auth] user: MyUser) -> Result<User, AppError> {
    ctx.get_user(&user.id).cloned().ok_or(AppError::NotFound)
}
```

> **Note:** `AuthedUser` is the built-in convention type — extractors find it
> automatically by type name, so you do not need to annotate it. For any
> *custom* user type, the explicit `#[auth]` parameter attribute is required:
>
> ```rust,ignore
> #[remote(query)]
> async fn me(ctx: &AppState, #[auth] user: MyUser) -> Result<MyUser, AppError> { ... }
> ```
>
> Without the attribute, `MyUser` would be treated as an ordinary
> deserializable input parameter. Use `Option<T>` (with or without `#[auth]`
> depending on the type) for optional authentication.

### Rejecting requests in auth

The `.auth()` validator returns `Option<U>` — `None` passes through
silently, letting extractors surface a plain `401 Unauthorized`. For
more control (for example, returning `403 Forbidden` for banned users),
use `.try_auth()` instead. The validator returns `Result<U, AppError<E>>`
and the middleware short-circuits the request with the error response:

```rust,ignore
use teleport::{AppError, TeleportRouter};

let app = TeleportRouter::new()
    .state(Arc::clone(&state))
    .try_auth("session", |token: String, state: Arc<AppState>| async move {
        match state.validate_session(&token).await {
            Some(user) if user.banned => Err(AppError::<()>::Forbidden),
            Some(user) => Ok(user),
            None => Err(AppError::<()>::Unauthorized),
        }
    })
    .mount();
```

See [`error-handling.md`](error-handling.md) for the full story on
`AppError<T>` variants, HTTP status mapping, and when to use `try_auth`
over `auth`.

## 7. Typed errors

Define procedure-specific error details:

```rust
use teleport::teleport_type;

#[teleport_type]
struct LoginErrorDetail {
    invalid_credentials: bool,
}

#[remote(command)]
async fn login(ctx: &AppState, input: LoginRequest) -> Result<LoginResponse, AppError<LoginErrorDetail>> {
    // ...
    Err(AppError::detail(LoginErrorDetail { invalid_credentials: true }))
}
```

On the TypeScript side, the error detail type flows through:

```typescript
const result = await auth.login({ email, password });
if (!result.ok && "error" in result) {
    if (result.error.detail?.invalid_credentials) {
        // Handle invalid credentials
    }
}
```

## 8. Dev workflow

Run the server with auto-regeneration:

```bash
cargo watch -x run
```

The Vite plugin picks up changes to the generated files and triggers HMR in your frontend.

## Next steps

- See `examples/demo/` for a complete working application
- See [architecture.md](architecture.md) for design decisions and crate structure
