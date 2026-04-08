# teleport-rs — Auth and Cookie Forwarding

## Architecture

```
Browser
  │
  │ Cookie: session_id=abc123
  ▼
SvelteKit BFF
  │
  │ Cookie: session_id=abc123  (auto-forwarded)
  │ Optional: Authorization: Bearer <token>  (explicit override)
  ▼
Rust Backend (Axum)
  │
  │ Middleware extracts session from cookie or Authorization header
  │ Sets AppState.current_user = Some(AuthedUser) or None
  ▼
Procedure Handler
```

## Auth Design

**Two modes, both supported:**

1. **Auto-forward cookies** — SvelteKit BFF proxies the browser's cookies to Rust. The Rust backend reads the session cookie. This is the default and requires zero configuration.

2. **Explicit token** — SvelteKit extracts the session, creates a JWT/ bearer token, and passes it via `Authorization` header to Rust. Used when cookies can't be forwarded (e.g., cross-origin, different domains).

## Rust Side: Auth Middleware

```rust
use axum::{middleware, extract::State, http::Request, body::Body};

#[derive(Debug, Clone)]
pub struct AuthedUser {
    pub id: String,
    pub email: String,
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    User,
    Guest,
}

/// Auth middleware that runs before every teleport-rs procedure.
/// Extracts session from cookie or Authorization header.
/// Sets current_user on AppState (per-request clone).
pub async fn auth_middleware(
    mut req: Request<Body>,
    next: middleware::Next<Body>,
) -> Response {
    // Try cookie first
    let session_id = req
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookie| {
            // Parse session_id from cookie string
            cookie.split(';')
                .find_map(|c| {
                    let c = c.trim();
                    c.strip_prefix("session_id=")
                })
        })
        .map(|s| s.to_string());

    // Try Authorization header second
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let token = session_id.or(auth_header);

    // Look up user from token (if present)
    let current_user = if let Some(token) = token {
        // Validate token and get user
        verify_session(&token).await.ok()
    } else {
        None
    };

    // Store in request extensions so handlers can access it
    req.extensions_mut().insert(current_user);

    next.run(req).await
}
```

## AppState with Auth

```rust
use std::sync::Arc;

pub struct AppState {
    pub db: sqlx::PgPool,
    pub redis: RedisPool,
    pub config: AppConfig,
}

// AuthedUser is stored per-request in extensions, NOT in AppState.
// AppState is shared across all requests. AuthedUser is per-request.

// Procedures access the authed user via Axum extensions:
```

**Revised approach** — Since `AppState` is shared (Arc), we cannot mutate it per-request. Instead, auth info goes into Axum request extensions:

```rust
// Procedures that need auth access it via request extensions
#[remote(query)]
async fn get_my_orders(ctx: &AppState, auth: &AuthedUser) -> Result<Vec<Order>, AppError<GetOrdersError>> {
    ctx.db.get_orders(&auth.id).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

**Wait** — this conflicts with the simple `ctx: &AppState` signature. Two approaches:

### Approach A: Extensions parameter (recommended)

The proc macro supports an optional `AuthedUser` parameter:

```rust
#[remote(query)]
async fn get_my_orders(ctx: &AppState, auth: AuthedUser, input: GetOrdersInput) -> Result<Vec<Order>, AppError<GetOrdersError>> {
    // auth is extracted from request extensions
    ctx.db.get_orders(&auth.id).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

The macro generates an Axum handler that:

1. Extracts `State<Arc<AppState>>`
2. Extracts `Extension<AuthedUser>` (returns 401 if missing)
3. Deserializes input
4. Calls your function with all parameters

### Approach B: Auth is a field on context (simpler but less type-safe)

```rust
// Each request gets a RequestCtx that wraps AppState + auth
pub struct RequestCtx {
    pub state: Arc<AppState>,
    pub current_user: Option<AuthedUser>,
}

#[remote(query)]
async fn get_my_orders(ctx: &RequestCtx) -> Result<Vec<Order>, AppError<GetOrdersError>> {
    let user = ctx.current_user.as_ref().ok_or(AppError::Unauthorized)?;
    ctx.state.db.get_orders(&user.id).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

### Decision: Approach A

Approach A is more explicit and type-safe. The parameter list clearly shows which procedures require auth:

- `async fn get_version(ctx: &AppState) -> Result<...>` → no auth needed
- `async fn get_my_orders(ctx: &AppState, auth: AuthedUser) -> Result<...>` → auth required, 401 if not logged in
- `async fn get_my_orders(ctx: &AppState, auth: Option<AuthedUser>) -> Result<...>` → auth optional

This matches the Axum extractor pattern and is immediately readable.

## Cookie Forwarding in SvelteKit

The `@teleport-rs/client` config defaults to `credentials: 'include'` which forwards cookies. For SvelteKit server-side calls (in remote functions), cookies are forwarded from the incoming request:

```typescript
// @teleport-rs/client — server-side configuration for SvelteKit

import { configure } from "@teleport-rs/client";

// In SvelteKit, remote functions run on the server.
// We need to forward the browser's cookies to the Rust backend.
// This is done by reading cookies from the SvelteKit request event
// and passing them in the rpc headers.

// $app/server provides getRequestEvent() which gives access to cookies
import { getRequestEvent } from "$app/server";

configure({
  baseUrl: "http://localhost:3000/rpc",
  headers: async () => {
    const event = getRequestEvent();
    const cookie = event.request.headers.get("cookie") || "";
    return { cookie };
  },
  credentials: "include",
});
```

## SvelteKit Remote Functions — Auth Pattern

```typescript
// src/lib/server/data.remote.ts

import { query, command, getRequestEvent } from "$app/server";
import { z } from "zod";
import { auth, users } from "$lib/api/generated/client";

// Public query — no auth required
export const getVersion = query(async () => {
  const result = await auth.getVersion();
  if (!result.ok) throw new Error("Failed to get version");
  return result.data;
});

// Authenticated query — cookies auto-forwarded by teleport client
export const getMyProfile = query(async () => {
  const result = await users.getMyProfile();
  if (!result.ok) {
    if ("transport" in result) throw new Error(result.transport.message);
    if (result.error.type === "Unauthorized") {
      // Redirect to login or throw
      throw new Error("Unauthorized");
    }
    throw new Error(result.error.type);
  }
  return result.data;
});

// Authenticated command — login is special because it SETS the cookie
export const login = command(
  z.object({ email: z.string(), password: z.string() }),
  async (input) => {
    const result = await auth.login(input);
    if (!result.ok) {
      if ("transport" in result) throw new Error(result.transport.message);
      if (result.error.type === "Detail") {
        if (result.error.detail.invalidCredentials) {
          return { success: false, error: "Invalid credentials" };
        }
      }
      throw new Error("Login failed");
    }
    // Set session cookie in SvelteKit
    // (the Rust backend returns the session token, SvelteKit sets the cookie)
    const event = getRequestEvent();
    event.cookies.set("session_id", result.data.token, {
      httpOnly: true,
      secure: true,
      sameSite: "lax",
      path: "/",
      maxAge: result.data.expiresIn,
    });
    return { success: true };
  },
);
```

## Auth Flow: Login → Subsequent Requests

```
1. Browser POSTs login form
2. SvelteKit remote function calls auth.login()
3. teleport-rs client POSTs to /rpc/auth.login (no cookie yet)
4. Rust backend validates credentials, returns AuthToken { token, expiresIn }
5. SvelteKit remote function receives AuthToken
6. SvelteKit sets session_id cookie with the token
7. Browser stores cookie

8. Browser requests page needing auth
9. SvelteKit remote function calls users.getMyProfile()
10. teleport-rs client reads browser cookie from getRequestEvent()
11. teleport-rs client includes Cookie: session_id=abc123 in request
12. Rust backend auth middleware extracts session_id from cookie
13. Middleware validates session, sets AuthedUser in extensions
14. Procedure handler receives AuthedUser parameter
15. Data returned normally
```

## Logout Flow

```rust
#[remote(command)]
async fn logout(ctx: &AppState, auth: AuthedUser) -> Result<(), AppError> {
    // Invalidate the session
    ctx.redis.del(format!("session:{}", auth.id)).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

```typescript
export const logout = command(async () => {
  const result = await auth.logout();
  const event = getRequestEvent();
  event.cookies.delete("session_id", { path: "/" });
  if (!result.ok) throw new Error("Logout failed");
});
```

## Explicit Token Override

For cases where you need to call Rust with a specific token (e.g., API keys, service-to-service):

```typescript
// @teleport-rs/client — per-call override
import { rpc } from "@teleport-rs/client";

const result = await rpc<AdminData, AdminError>(
  "POST",
  "/rpc/admin.getData",
  { query: "something" },
  { headers: { Authorization: `Bearer ${serviceToken}` } },
);
```

The rpc function signature extends to:

```typescript
export async function rpc<T, E>(
  method: HttpMethod,
  path: string,
  input: unknown,
  options?: { headers?: Record<string, string>; timeout?: number },
): Promise<RpcResult<T, E>>;
```
