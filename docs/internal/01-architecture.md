# teleport-rs — Architecture

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Monorepo                                                       │
│                                                                 │
│  ┌──────────────────┐         ┌────────────────────────────┐  │
│  │  rust-server/     │         │  sveltekit-frontend/        │  │
│  │                   │         │                             │  │
│  │  src/api/         │  gen    │  src/lib/api/               │  │
│  │  ┌─────────────┐  │ ──────► │  ┌─────────────────────┐   │  │
│  │  │ #[remote]   │  │        │  │  generated/          │   │  │
│  │  │ procedures  │  │        │  │  ├─ types.ts          │   │  │
│  │  └─────────────┘  │        │  │  ├─ client.ts         │   │  │
│  │                   │        │  │  └─ errors.ts         │   │  │
│  │  ┌─────────────┐  │        │  └─────────────────────┘   │  │
│  │  │ AppState    │  │        │                             │  │
│  │  │ (DB, etc.)  │  │        │  src/routes/                │  │
│  │  └─────────────┘  │        │  ┌─────────────────────┐   │  │
│  │                   │        │  │  data.remote.ts      │   │  │
│  │  ┌─────────────┐  │        │  │  (handwritten)      │   │  │
│  │  │ Teleport    │  │        │  └─────────────────────┘   │  │
│  │  │ Router      │◄─┼────────┼───────── HTTP ────────────┘  │
│  │  └─────────────┘  │        │                               │
│  └──────────────────┘         └────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Request Flow

### Query (read-only, GET)

```
1. Browser calls getLikesRemote(itemId) in SvelteKit component
2. SvelteKit remote function validates input with Zod
3. Remote function calls generated client: getUser(itemId)
4. Client serializes args, sends GET /rpc/users.getUser?id=123
5. Axum receives request, auth middleware injects session
6. #[remote(query)] handler runs with &AppState
7. Handler returns Result<User, AppError<GetUserError>>
8. Teleport serializes response as JSON
9. Client deserializes into Result<User, TransportOrAppError<GetUserError>>
10. Remote function returns data to browser
```

### Command (mutation, POST)

```
1. Browser calls loginRemote({ email, password })
2. SvelteKit remote function validates with Zod
3. Remote function calls: login({ email, password })
4. Client serializes, sends POST /rpc/auth.login with JSON body
5. Axum receives, auth middleware forwards cookies
6. #[remote(command)] handler runs with &AppState
7. Handler returns Result<AuthToken, AppError<LoginError>>
8-10. Same as query
```

### Form (form mutation, POST, progressive enhancement)

```
1. Browser submits <form {...loginForm}>
2. SvelteKit remote function (form type) receives FormData
3. Remote function calls: login({ email, password })
4-8. Same as command
9. Client returns result to remote function
10. Remote function returns { success: true } etc.
```

## Route Naming Convention

Rust function names are namespaced by module path and converted to camelCase:

```rust
// src/api/users.rs
#[remote(query)]
async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError<GetUserError>> {}

#[remote(query)]
async fn get_posts(ctx: &AppState, user_id: String) -> Result<Vec<Post>, AppError<GetPostError>> {}
```

Generates routes:

- `GET /rpc/users.getUser?id=...`
- `GET /rpc/users.getPosts?userId=...`

And TS client:

```typescript
export const users = {
  getUser: (id: string) =>
    rpc<User, AppError<GetUserDetail>>("GET", "/rpc/users.getUser", { id }),
  getPosts: (userId: string) =>
    rpc<Vec<Post>, AppError<GetPostDetail>>("GET", "/rpc/users.getPosts", {
      userId,
    }),
};
```

Optional override:

```rust
#[remote(query, name = "fetchProfile")]
async fn get_user_profile(...) -> ... {}
```

## Serialization

All data between SvelteKit BFF and Rust backend is JSON, using serde for Rust and native JSON.parse/stringify for TS.

### Rust → TS type mapping

| Rust                             | TypeScript                      |
| -------------------------------- | ------------------------------- |
| `String`                         | `string`                        |
| `i32`, `i64`, `u32`, `u64`       | `number`                        |
| `f64`                            | `number`                        |
| `bool`                           | `boolean`                       |
| `Vec<T>`                         | `Array<T>`                      |
| `Option<T>`                      | `T \| null`                     |
| `HashMap<K, V>`                  | `Record<K, V>`                  |
| `Result<T, E>` (in response)     | Handled by teleport-rs protocol |
| `chrono::DateTime<Utc>`          | `string` (ISO 8601)             |
| `uuid::Uuid`                     | `string`                        |
| `serde_json::Value`              | `unknown`                       |
| Custom `#[derive(Type)]` structs | Corresponding TS interface      |

Specta handles all type conversion. Custom types need `#[derive(Type)]` + `#[derive(Serialize, Deserialize)]`.

## Axum Integration

teleport-rs generates an Axum `Router` from all `#[remote]` annotations:

```rust
use teleport_rs::TeleportRouter;
use axum::Router;

let app = Router::new()
    .merge(TeleportRouter::new()
        .state(app_state)
        .mount()
    );
```

This produces a router with all `/rpc/*` routes registered, with auth middleware applied.

## Monorepo Structure

```
teleport-rs/
├── crates/
│   ├── teleport/              # Core library (proc macros, router, error, auth)
│   ├── teleport-macros/       # Proc macro implementation (#[remote])
│   └── teleport-build/        # Build-time TS generation logic
├── packages/
│   ├── client/                 # @teleport-rs/client (npm)
│   └── vite/                   # @teleport-rs/vite (npm)
├── examples/
│   └── sveltekit-demo/         # Full working example
├── plan/                       # This planning directory
├── Cargo.toml                  # Workspace
└── package.json                # Workspace (for npm packages)
```
