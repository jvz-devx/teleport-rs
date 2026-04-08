# teleport-rs — Full Usage Examples

## Example 1: Simple Query

### Rust

```rust
// src/api/users.rs

use teleport_rs::{remote, AppError};
use crate::state::AppState;
use crate::types::User;
use crate::errors::GetUserErrorDetail;

#[remote(query)]
async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError<GetUserErrorDetail>> {
    ctx.db
        .get_user(&id)
        .await
        .ok_or(AppError::NotFound)
}
```

### Generated TypeScript

```typescript
// generated/client.ts
export const users = {
  getUser: (id: string): Promise<RpcResult<User, GetUserErrorDetail>> =>
    rpc("GET", "/rpc/users.getUser", { id }),
};
```

### SvelteKit Remote Function

```typescript
// src/lib/server/data.remote.ts

import { query } from "$app/server";
import { z } from "zod";
import { users } from "$lib/api/generated/client";

export const getUser = query(z.string(), async (id) => {
  const result = await users.getUser(id);

  if (!result.ok) {
    if ("transport" in result) {
      throw new Error(`Network error: ${result.transport.message}`);
    }
    if (result.error.type === "NotFound") {
      throw new Error("User not found");
    }
    throw new Error(`Unexpected error: ${result.error.type}`);
  }

  return result.data;
});
```

### Svelte Component

```svelte
<!-- src/routes/users/[id]/+page.svelte -->

<script>
  import { getUser } from './data.remote';
  let { data } = $props();
</script>

{#await getUser(data.id)}
  <p>Loading...</p>
{:then user}
  <h1>{user.name}</h1>
  <p>{user.email}</p>
{:catch error}
  <p>Error: {error.message}</p>
{/await}
```

---

## Example 2: Command with Auth

### Rust

```rust
// src/api/auth.rs

use teleport_rs::{remote, AppError};
use crate::state::AppState;
use crate::types::{LoginRequest, AuthToken};
use crate::errors::LoginErrorDetail;

#[remote(command)]
async fn login(ctx: &AppState, req: LoginRequest) -> Result<AuthToken, AppError<LoginErrorDetail>> {
    match ctx.auth.login(&req.email, &req.password).await {
        Ok(token) => Ok(token),
        Err(AuthError::InvalidCredentials) => Err(AppError::Detail(LoginErrorDetail {
            invalid_credentials: true,
            account_locked: false,
        })),
        Err(AuthError::AccountLocked) => Err(AppError::Detail(LoginErrorDetail {
            invalid_credentials: false,
            account_locked: true,
        })),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

#[remote(command)]
async fn logout(ctx: &AppState, auth: AuthedUser) -> Result<(), AppError> {
    ctx.auth.logout(&auth.id).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

### SvelteKit Remote Function

```typescript
// src/lib/server/data.remote.ts

import { command } from "$app/server";
import { z } from "zod";
import { auth } from "$lib/api/generated/client";

export const login = command(
  z.object({ email: z.string().email(), password: z.string().min(8) }),
  async (input) => {
    const result = await auth.login(input);

    if (!result.ok) {
      if ("transport" in result) {
        throw new Error(`Network error: ${result.transport.message}`);
      }
      if (result.error.type === "Detail") {
        const { invalidCredentials, accountLocked } = result.error.detail;
        if (invalidCredentials) {
          return {
            success: false as const,
            error: "Invalid email or password",
          };
        }
        if (accountLocked) {
          return {
            success: false as const,
            error: "Account is locked. Please contact support.",
          };
        }
      }
      throw new Error("Login failed");
    }

    // Set session cookie
    const event = getRequestEvent();
    event.cookies.set("session_id", result.data.token, {
      httpOnly: true,
      secure: true,
      sameSite: "lax",
      path: "/",
      maxAge: result.data.expiresIn,
    });

    return { success: true as const };
  },
);

export const logout = command(async () => {
  const result = await auth.logout();

  if (!result.ok) {
    // Even if logout fails on the server, clear the cookie locally
  }

  const event = getRequestEvent();
  event.cookies.delete("session_id", { path: "/" });

  return { success: true };
});
```

### Svelte Component

```svelte
<!-- src/routes/auth/login/+page.svelte -->

<script>
  import { login } from './data.remote';

  let email = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    loading = true;
    error = '';

    const result = await login({ email, password });

    if (!result.success) {
      error = result.error;
      loading = false;
      return;
    }

    window.location.href = '/';
  }
</script>

<form on:submit={handleSubmit}>
  <input type="email" bind:value={email} placeholder="Email" required />
  <input type="password" bind:value={password} placeholder="Password" required />
  <button type="submit" disabled={loading}>
    {loading ? 'Logging in...' : 'Log in'}
  </button>
</form>

{#if error}
  <p class="error">{error}</p>
{/if}
```

---

## Example 3: Form Submission

### Rust

```rust
// src/api/posts.rs

use teleport_rs::{remote, AppError};
use crate::state::AppState;
use crate::types::{CreatePostRequest, Post};
use crate::errors::CreatePostErrorDetail;

#[remote(form)]
async fn create_post(ctx: &AppState, input: CreatePostRequest) -> Result<Post, AppError<CreatePostErrorDetail>> {
    if input.title.len() < 3 {
        return Err(AppError::Detail(CreatePostErrorDetail {
            title_too_short: true,
            content_too_long: false,
        }));
    }
    if input.content.len() > 10_000 {
        return Err(AppError::Detail(CreatePostErrorDetail {
            title_too_short: false,
            content_too_long: true,
        }));
    }
    ctx.db.create_post(input).await.map_err(|e| AppError::Internal(e.to_string()))
}
```

### SvelteKit Remote Function

```typescript
// src/lib/server/data.remote.ts

import { form } from "$app/server";
import { posts } from "$lib/api/generated/client";

export const createPost = form(async (formData: FormData) => {
  const input = {
    title: formData.get("title") as string,
    content: formData.get("content") as string,
  };

  const result = await posts.createPost(input);

  if (!result.ok) {
    if ("transport" in result) {
      return { success: false as const, error: "Network error" };
    }
    if (result.error.type === "Detail") {
      const { titleTooShort, contentTooLong } = result.error.detail;
      return {
        success: false as const,
        error: [
          titleTooShort && "Title must be at least 3 characters",
          contentTooLong && "Content must be less than 10,000 characters",
        ]
          .filter(Boolean)
          .join(". "),
      };
    }
    return { success: false as const, error: "Failed to create post" };
  }

  return { success: true as const, post: result.data };
});
```

### Svelte Component (Progressive Enhancement)

```svelte
<!-- src/routes/posts/new/+page.svelte -->

<script>
  import { createPost } from './data.remote';
</script>

<form {...createPost.enhance(async ({ submit }) => {
  try {
    const result = await submit().updates();
    if (result.success) {
      // Redirect to the new post
    }
  } catch (error) {
    // Show error
  }
})}>
  <input type="text" name="title" placeholder="Title" required />
  <textarea name="content" placeholder="Write your post..." required></textarea>
  <button type="submit">Create Post</button>
</form>

{#if createPost.result}
  {#if !createPost.result.success}
    <p class="error">{createPost.result.error}</p>
  {/if}
{/if}
```

---

## Example 4: Authenticated Query

### Rust

```rust
// src/api/users.rs

use teleport_rs::{remote, AppError};
use crate::state::AppState;
use crate::auth::AuthedUser;
use crate::types::UserProfile;
use crate::errors::*;

#[remote(query)]
async fn get_my_profile(ctx: &AppState, auth: AuthedUser) -> Result<UserProfile, AppError> {
    ctx.db
        .get_user_profile(&auth.id)
        .await
        .ok_or(AppError::NotFound)
}

#[remote(query)]
async fn get_public_profile(ctx: &AppState, id: String) -> Result<UserProfile, AppError> {
    // No AuthedUser parameter — this is a public endpoint
    ctx.db
        .get_user_profile(&id)
        .await
        .ok_or(AppError::NotFound)
}
```

Generated routes:

- `GET /rpc/users.getMyProfile` — requires auth (401 if not authenticated)
- `GET /rpc/users.getPublicProfile?id=...` — public, no auth required

The `AuthedUser` parameter is extracted by the auth middleware. If the session cookie is missing or invalid, the middleware returns 401 before the procedure runs.

---

## Example 5: Full App State Setup

### Rust

```rust
// src/state.rs

use std::sync::Arc;
use sqlx::PgPool;
use redis::aio::ConnectionManager;

pub struct AppState {
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub config: AppConfig,
}

pub struct AppConfig {
    pub jwt_secret: String,
    pub database_url: String,
}
```

```rust
// src/main.rs

use axum::Router;
use teleport_rs::TeleportRouter;
use std::sync::Arc;

mod api;
mod state;
mod auth;
mod errors;
mod types;

use state::AppState;

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState::new().await);

    let app = Router::new()
        .merge(
            TeleportRouter::new()
                .state(state)
                .with_auth(auth::auth_middleware)
                .mount()
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### SvelteKit Configuration

```typescript
// src/lib/api/config.ts

import { configure } from "@teleport-rs/client";

configure({
  baseUrl: import.meta.env.VITE_RPC_URL || "http://localhost:3000/rpc",
  timeout: 10000,
  credentials: "include",
});
```

```typescript
// src/lib/api/index.ts

export type {
  User,
  LoginRequest,
  AuthToken,
  Post,
  CreatePostRequest,
} from "./generated/types";
export type { AppError, TransportError, RpcResult } from "./generated/errors";
export type {
  LoginError,
  GetUserError,
  CreatePostError,
} from "./generated/errors";
export { auth, users, posts } from "./generated/client";
export {
  configure,
  isTransportError,
  isAppError,
  unwrap,
} from "@teleport-rs/client";
```

```typescript
// svelte.config.js

import adapter from "@sveltejs/adapter-auto";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { teleportVite } from "@teleport-rs/vite";

export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter(),
    vite: {
      plugins: [
        teleportVite({
          bindingsPath: "../rust-server/bindings",
          outputPath: "src/lib/api/generated",
          generateOnStart: true,
        }),
      ],
    },
  },
};
```

---

## Example 6: Error Handling Patterns

### Exhaustive Pattern Matching

```typescript
// In a SvelteKit remote function
const result = await users.getUser(id);

if (result.ok) {
  return result.data; // User
}

// TypeScript enforces handling both transport and app errors
if ("transport" in result) {
  // Transport error — network/timeout
  throw new Error(`Network error: ${result.transport.message}`);
}

// result.error is AppError<GetUserErrorDetail>
switch (result.error.type) {
  case "Unauthorized":
    throw redirect(302, "/login");
  case "NotFound":
    throw error(404, "User not found");
  case "Detail":
    // TypeScript knows detail is GetUserErrorDetail here
    if (result.error.detail.userNotFound) {
      throw error(404, "User not found");
    }
    throw error(500, "Unexpected error");
  case "BadRequest":
  case "Internal":
  case "RateLimited":
  case "Forbidden":
    throw error(500, result.error.type);
}
```

### Using Helper Functions

```typescript
import { unwrap } from "@teleport-rs/client";

// Throw on any error (simple cases)
const user = unwrap(await users.getUser(id));
// user is typed as User, throws on error

// Check for specific errors
import { isAppError, isTransportError } from "@teleport-rs/client";

const result = await users.getUser(id);
if (isTransportError(result)) {
  // result is { ok: false; transport: TransportError }
  throw new Error("Network error");
}
if (isAppError(result)) {
  // result is { ok: false; error: AppError<GetUserErrorDetail> }
  if (result.error.type === "NotFound") {
    throw error(404);
  }
}
// result is { ok: true; data: User }
return result.data;
```
