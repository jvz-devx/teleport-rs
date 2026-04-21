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
http = "1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
specta = { version = "=2.0.0-rc.24", features = ["derive"] }
tower-http = { version = "0.6", features = ["cors", "limit", "catch-panic"] }
```

Why each one:

- `teleport` — the framework itself. The `export` feature enables the
  TypeScript generator bundled with the crate.
- `axum` — the HTTP server teleport-rs builds on. `TeleportRouter` is a
  thin wrapper around `axum::Router`.
- `http` — the low-level HTTP primitives (`HeaderValue`, `Method`, etc.)
  used by the CORS layer example later in this guide.
- `tokio` — async runtime.
- `serde` — required in scope because `#[teleport_type]` expands to
  `#[derive(serde::Serialize, serde::Deserialize)]`.
- `specta` — **pinned to an exact rc** because the specta proc-macro
  output must line up with the version `teleport-core` was built
  against. The `#[teleport_type]` macro expands to
  `#[derive(specta::Type)]`, so `specta` needs to be in scope in the
  consumer crate. This will loosen when specta stabilises.
- `tower-http` — the `limit` and `catch-panic` features are required
  because `TeleportRouter::mount()` uses them internally for the
  default safety layers. `cors` is for the CORS layer you will almost
  certainly want in production.

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

> **`#[teleport_type]` expands to `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]`. Do NOT add any of these derives yourself — you will hit E0119 (conflicting implementations).** If you need extras like `PartialEq`, `Eq`, or `Hash`, put them on a *separate* `#[derive(...)]` line alongside `#[teleport_type]`.

## 3. Defining AppState

`TeleportRouter<S>` is generic over your application state type `S`, and
has a hard bound:

```text
S: Clone + Send + Sync + 'static
```

The router clones `S` on every request before handing it to your
procedure, so the state struct itself must be **cheap to clone**. The
naive first attempt — wrapping the whole state in `Arc<Mutex<_>>` — does
not work, because procedures take `ctx: &AppState`, not
`ctx: &Arc<Mutex<AppState>>`, and the clone-on-every-request path would
serialize every handler behind a single mutex.

> **Wrap mutable fields in `Arc<Mutex<_>>` _inside_ the struct, not around the struct. `TeleportRouter` clones `S` on every request, so the whole struct must be cheap to clone.**

A working `AppState` with mutable data looks like this:

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use teleport::AuthedUser;

use crate::types::{Post, User};

#[derive(Debug, Clone)]
pub struct AppState {
    // Immutable after startup — no wrapper needed.
    users: Vec<User>,
    // Mutable — wrap the field, not the struct.
    posts: Arc<Mutex<Vec<Post>>>,
    // Another mutable shared counter.
    next_post_id: Arc<Mutex<u32>>,
    // Sessions keyed by cookie token.
    sessions: HashMap<String, AuthedUser>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            users: Vec::new(),
            posts: Arc::new(Mutex::new(Vec::new())),
            next_post_id: Arc::new(Mutex::new(1)),
            sessions: HashMap::new(),
        }
    }

    pub fn list_users(&self) -> &[User] {
        &self.users
    }

    pub fn get_user(&self, id: &str) -> Option<&User> {
        self.users.iter().find(|u| u.id == id)
    }

    pub fn add_post(&self, post: Post) {
        let mut posts = self.posts.lock().unwrap();
        posts.push(post);
    }

    pub fn list_posts(&self) -> Vec<Post> {
        self.posts.lock().unwrap().clone()
    }
}
```

`#[derive(Clone)]` works because every field is cheaply cloneable: `Vec`
is a plain clone, and `Arc<Mutex<_>>` clones just bump a refcount. The
`HashMap<String, AuthedUser>` is fine if it's only mutated at startup;
if you need to mutate it at runtime, wrap it the same way:
`Arc<Mutex<HashMap<String, AuthedUser>>>`.

`examples/demo/src/state.rs` in this repository is a full, working
reference.

## 4. Write procedures

Create `src/api/users.rs`:

```rust
use teleport::{remote, teleport_type, AppError};
use crate::state::AppState;
use crate::types::User;

#[teleport_type]
pub struct GetUserInput {
    pub id: String,
}

#[remote(query)]
async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> {
    ctx.get_user(&input.id).cloned().ok_or(AppError::NotFound)
}

#[remote(query)]
async fn list_users(ctx: &AppState) -> Result<Vec<User>, AppError> {
    Ok(ctx.list_users().to_vec())
}
```

> **Query inputs must be struct wrappers. `serde_qs` cannot deserialize bare primitive types — even a single-field input needs its own struct.** A signature like `async fn get_user(ctx: &AppState, id: String)` will compile but fail at runtime because the query-string deserializer expects a map shape.

Procedure types:

| Annotation | HTTP Method | Input Source | Use Case |
|---|---|---|---|
| `#[remote(query)]` | GET | Query params | Read-only fetches |
| `#[remote(command)]` | POST | JSON body | Mutations |
| `#[remote(form)]` | POST | Form-urlencoded or JSON | Progressive enhancement |

### Procedure namespaces

The TypeScript name of a procedure is `{namespace}.{fnName}`:

- `fnName` is the Rust function name converted to `camelCase`
  (so `get_user` becomes `getUser`).
- `namespace` defaults to the Rust module path where the `#[remote]`
  function lives — specifically, the last segment of `module_path!()`.

For a procedure defined in `src/api/users.rs`, the module path is
`my_app::api::users` and the generated TS binding is `users.getUser`.
That is exactly what you usually want, and it is the recommended layout
for any project with more than a handful of procedures: split
procedures into submodules like `src/api/users.rs`, `src/api/posts.rs`,
etc., and let the natural Rust module structure give you clean
namespaces without any manual overrides.

For a **single-file** app where `fn get_user` lives in `src/main.rs`,
the module path is just the crate name, so the binding comes out as
`my_app.getUser` — which is almost never what you want. To fix this,
pass an explicit `prefix`:

```rust
// Single-file app in src/main.rs.
// Without override: TypeScript client exposes `my_app.getUser`.
#[remote(query)]
async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> { /* ... */ }

// With override: TypeScript client exposes `users.getUser`.
#[remote(query, prefix = "users", name = "getUser")]
async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> { /* ... */ }
```

The two supported attribute keys are:

- `prefix = "..."` — replaces the default module-path namespace.
- `name = "..."` — replaces the default `camelCase(fn_ident)` name.

Either, both, or neither may be present. Use them when the natural
module structure does not match the desired public API.

## 5. Set up the router

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
- `client.ts` — explicit-client RPC helpers plus `bindClient(client)` for namespaced frontend calls
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

## 6. Set up the frontend

Install the frontend packages:

```bash
npm install @teleport-rs/client @teleport-rs/vite
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
import { createClient } from "@teleport-rs/client";
import { bindClient } from "./generated/client";

const client = createClient({
    baseUrl: "http://localhost:3000",
    credentials: "include",
});

const { users } = bindClient(client);

const result = await users.getUser({ id: "123" });
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

## 7. Adding authentication

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

### Using a custom user type

`AuthedUser` is the built-in convention type: it has an `{ id: String,
email: String }` shape and teleport-rs recognises it by name so that
`#[remote]` handlers can take `user: AuthedUser` directly without the
`#[auth]` attribute. It also ships with all the trait impls needed for
extraction.

For any user type with a *different* shape — say, one with
`roles: Vec<String>` or a `banned: bool` flag — **you must implement
two traits manually**:

1. `teleport::TeleportUser` — a marker trait that says "this type may
   be used as an auth extractor in a `#[remote]` procedure". The trait
   is defined in `crates/teleport/src/extractors.rs` as:

    ```rust,ignore
    pub trait TeleportUser: Clone + Send + Sync + 'static {}
    ```

    An empty impl is enough.

2. `axum::extract::FromRequestParts<S>` — tells axum how to pull the
   user out of request extensions. The auth middleware inserts the
   validated user into `Extensions` before the handler runs; your impl
   just fetches it back out. Follow the pattern used by `AuthedUser`
   itself in `crates/teleport/src/extractors.rs`.

Here is the full boilerplate for a custom `MyUser`:

```rust,ignore
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use teleport::{AppError, TeleportUser};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MyUser {
    pub id: String,
    pub email: String,
    pub roles: Vec<String>,
    pub banned: bool,
}

impl TeleportUser for MyUser {}

impl<S> FromRequestParts<S> for MyUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(AppError::Unauthorized)
    }
}
```

Once those two impls exist, `#[auth] user: MyUser` in a `#[remote]`
signature works exactly like `AuthedUser`, and `.try_auth(...)` can
short-circuit the request with a typed `AppError`. See
`crates/teleport/src/extractors.rs` for the reference implementation of
`AuthedUser` that this boilerplate mirrors.

> This boilerplate is currently manual. A `#[derive(TeleportUser)]`
> helper that generates both impls may ship in a future release.

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

## 8. Typed errors

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
import { createClient } from "@teleport-rs/client";
import { bindClient } from "./generated/client";

const client = createClient({ baseUrl: "http://localhost:3000" });
const { auth } = bindClient(client);

const result = await auth.login({ email, password });
if (!result.ok && "error" in result) {
    if (result.error.detail?.invalid_credentials) {
        // Handle invalid credentials
    }
}
```

## 9. Dev workflow

Run the server with auto-regeneration:

```bash
cargo watch -x run
```

The Vite plugin picks up changes to the generated files and triggers HMR in your frontend.

## Next steps

- See `examples/demo/` for a complete working application
- See [architecture.md](architecture.md) for design decisions and crate structure
