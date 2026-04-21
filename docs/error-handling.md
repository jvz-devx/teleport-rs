# Error Handling

teleport-rs has a single error type, `AppError<T>`, that flows end-to-end
from Rust procedures to TypeScript call sites. This document covers the
variants, how they map to HTTP, how to attach typed procedure-specific
details, and how to reject authentication with a custom response.

## The `AppError<T>` enum

`AppError<T>` lives in `crates/teleport/src/error.rs`. It has seven
variants. The generic parameter `T` is the procedure-specific *detail*
type and defaults to `()`.

```rust,ignore
pub enum AppError<T = ()> {
    Unauthorized,
    Forbidden,
    NotFound,
    BadRequest { message: String },
    Internal { message: String },
    RateLimited,
    Detail { detail: T },
}
```

### HTTP status mapping

| Variant          | HTTP status | When to use |
|------------------|-------------|-------------|
| `Unauthorized`   | `401` | No valid session / authentication required but absent. |
| `Forbidden`      | `403` | Authenticated but lacks permission for this action. |
| `NotFound`       | `404` | The requested resource does not exist. |
| `BadRequest`     | `400` | Input validation failed. `message` is human-readable. |
| `Internal`       | `500` | Unexpected server error. `message` is for logs, not end users. |
| `RateLimited`    | `429` | Too many requests; client should back off. |
| `Detail { detail: T }` | `422` | Procedure-specific error typed by `T`. |

The `422 Unprocessable Entity` status for `Detail` is deliberate: the
request was syntactically valid, but the business logic rejected it
(e.g. "email already registered"). Use `BadRequest` for schema-level
problems and `Detail` for domain-level rejections.

The mapping is defined on `AppError::status_code` in
`crates/teleport/src/error.rs`. Serialization uses `serde(tag = "type")`,
so the JSON wire format is
`{"type":"NotFound"}`, `{"type":"BadRequest","message":"..."}`,
`{"type":"Detail","detail":{...}}`, etc.

## Choosing a variant

A rough decision tree:

```
Did the request itself fail to parse or validate?
  └─ Yes → BadRequest { message }

Is the user not authenticated?
  └─ Yes → Unauthorized

Is the user authenticated but not allowed?
  └─ Yes → Forbidden

Does the resource exist?
  └─ No → NotFound

Is this a transient overload / rate limit?
  └─ Yes → RateLimited

Is this a domain-specific rejection the client needs to react to
specifically (show a custom form error, resubmit, etc.)?
  └─ Yes → Detail { detail: MyErrorDetail }

Otherwise, if something went wrong server-side:
  └─ Internal { message }  (log the message; never leak sensitive info)
```

`Internal { message }` should treat `message` as internal-only. Don't
embed database rows, secrets, or stack traces in it — the JSON body is
returned to the client. A short breadcrumb for your logs is fine.

## Typed error details

The `Detail { detail: T }` variant is the escape hatch for
procedure-specific errors. Any `#[teleport_type]` struct can serve as
the detail type:

```rust,ignore
use teleport::{remote, teleport_type, AppError};

#[teleport_type]
pub struct LoginErrorDetail {
    pub invalid_credentials: bool,
    pub retry_after: Option<u64>,
}

#[remote(command)]
async fn login(
    ctx: &AppState,
    input: LoginRequest,
) -> Result<LoginResponse, AppError<LoginErrorDetail>> {
    if !ctx.verify_password(&input.email, &input.password) {
        return Err(AppError::detail(LoginErrorDetail {
            invalid_credentials: true,
            retry_after: None,
        }));
    }
    if ctx.is_throttled(&input.email) {
        return Err(AppError::detail(LoginErrorDetail {
            invalid_credentials: false,
            retry_after: Some(60),
        }));
    }
    Ok(ctx.issue_session(&input.email))
}
```

Only one `T` is supported per procedure. If you need to distinguish
several failure modes, you can model `T` as either an enum with
variant-specific fields (see below) or a flat struct with boolean
flags. Both patterns are supported.

### Enum error details (discriminated unions)

Rust enums with struct or tuple variants are fully supported as typed
error detail types. teleport-rs post-processes `specta-typescript`'s
enum rendering to emit the correct externally-tagged TypeScript shape,
preserving variant names and the nesting level that matches serde's
wire format.

```rust,ignore
#[teleport_type]
pub enum CreateLinkError {
    SlugTaken,
    SlugInvalid { reason: String },
    UrlInvalid { reason: String },
}

#[remote(command)]
async fn create_link(
    ctx: &AppState,
    input: CreateLinkInput,
) -> Result<ShortLink, AppError<CreateLinkError>> {
    if ctx.slug_exists(&input.slug).await {
        return Err(AppError::detail(CreateLinkError::SlugTaken));
    }
    if !input.slug.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::detail(CreateLinkError::SlugInvalid {
            reason: "only alphanumerics allowed".into(),
        }));
    }
    // ...
    # unreachable!()
}
```

The wire format (serde default, externally tagged):

```json
{"type":"Detail","detail":"SlugTaken"}
{"type":"Detail","detail":{"SlugInvalid":{"reason":"only alphanumerics allowed"}}}
```

The generated TypeScript:

```typescript
export type CreateLinkError =
    | "SlugTaken"
    | { SlugInvalid: { reason: string } }
    | { UrlInvalid: { reason: string } };
```

TypeScript consumers narrow by checking for the variant name key:

```typescript
if (result.error.type === "Detail") {
    const detail = result.error.detail;
    if (detail === "SlugTaken") {
        showSlugTakenError();
    } else if ("SlugInvalid" in detail) {
        showSlugInvalidError(detail.SlugInvalid.reason);
    } else if ("UrlInvalid" in detail) {
        showUrlInvalidError(detail.UrlInvalid.reason);
    }
}
```

The `"X" in detail` key check is the correct narrowing pattern for
externally-tagged discriminated unions — each variant object has the
variant name as its single key, so `in` both proves the variant AND
gives TypeScript enough information to type the contents.

#### `#[serde(tag = "...")]` is not supported

Adding `#[serde(tag = "kind")]` to a `#[teleport_type]` enum changes the
Rust wire format to internally-tagged
`{"kind":"SlugInvalid","reason":"..."}`, but `specta-typescript` 0.0.11
does not expose that attribute to its renderer. teleport-rs cannot
detect the intent and always renders externally-tagged, regardless of
the serde attribute. If you set `#[serde(tag)]` on a
`#[teleport_type]` enum, the Rust wire format and the generated
TypeScript type will diverge. Don't do it. If you need internal
tagging, use a flat struct with an enum-valued `kind: Reason` field
where `Reason` is a unit-only enum (which renders cleanly as a TS
string literal union).

### Flat struct error details (alternative)

If you prefer a flat struct to an enum for any reason — simpler client
narrowing, multiple simultaneous flags, easier to extend without
breaking — that pattern still works:

```rust,ignore
#[teleport_type]
pub struct CreateLinkError {
    pub slug_taken: bool,
    pub slug_invalid: Option<String>,
    pub url_invalid: Option<String>,
}
```

TypeScript side:

```typescript
if (result.error.type === "Detail") {
    if (result.error.detail.slug_taken) showSlugTakenError();
    else if (result.error.detail.slug_invalid !== null) {
        showSlugInvalidError(result.error.detail.slug_invalid);
    }
}
```

Both patterns are first-class — pick whichever matches your error
vocabulary better.

Procedures that do not need typed details keep the default and use
`AppError` (i.e. `AppError<()>`):

```rust,ignore
#[teleport_type]
pub struct GetUserInput {
    pub id: String,
}

async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> {
    ctx.get_user(&input.id).cloned().ok_or(AppError::NotFound)
}
```

## TypeScript side

The generated `errors.ts` defines `AppError<E>` as a discriminated union
whose `type` tag matches the Rust enum. The client package exports type
guards and helpers in `packages/client/src/result.ts`:

```typescript
import { createClient } from "@teleport-rs/client";
import { isAppError, isTransportError, rpcUnwrap } from "@teleport-rs/client";
import { bindClient } from "./generated/client";
import type { LoginErrorDetail } from "./generated/types";

const client = createClient({ baseUrl: "http://localhost:3000" });
const { auth } = bindClient(client);

const result = await auth.login({ email, password });

if (isAppError(result)) {
    // result.error: AppError<LoginErrorDetail>
    if (result.error.type === "Detail" && result.error.detail.invalid_credentials) {
        showInvalidCredentials();
        return;
    }
    if (result.error.type === "Unauthorized") {
        redirectToLogin();
        return;
    }
}

if (isTransportError(result)) {
    // result.transport: { type: "NetworkError" | "Timeout" | "ServerError", ... }
    console.error("network issue", result.transport);
    return;
}

// Success path: result.data is fully typed.
console.log(result.data.token);
```

For contexts where throwing is acceptable (e.g. inside a SvelteKit
remote function), use `rpcUnwrap`:

```typescript
import { createClient, rpcUnwrap, TeleportError } from "@teleport-rs/client";
import { bindClient } from "./generated/client";

const client = createClient({ baseUrl: "http://localhost:3000" });
const { auth } = bindClient(client);

try {
    const session = rpcUnwrap(await auth.login({ email, password }));
    return session;
} catch (err) {
    if (err instanceof TeleportError && err.is("Detail")) {
        // err.detail is the LoginErrorDetail
    }
    throw err;
}
```

`TeleportError` carries the original `AppError<E>` so you can inspect
`.detail` and `.is(variant)` without losing type safety. `.is(...)`
only accepts known teleport error variants; for arbitrary string
comparisons, use `err.appError.type` directly.

Transport errors (network failures, timeouts, non-JSON responses) are
surfaced through `isTransportError` or via `TransportFailure` from
`rpcUnwrap`. They are always separate from `AppError` so a caller can
distinguish "the server said no" from "I couldn't reach the server".

## `try_auth` vs `auth`: rejecting with a custom error

`TeleportRouter::auth(cookie, |token, state| async { Option<U> })` is
the infallible variant. If the validator returns `None`, the middleware
passes through silently and any procedure that asks for an `AuthedUser`
or a `#[auth]`-annotated user returns `401 Unauthorized` from the
extractor. This is the right default for almost everything.

`TeleportRouter::try_auth(cookie, |token, state| async { Result<U, AppError<E>> })`
is the fallible variant added in this release. The validator can return
any `AppError<E>` and the middleware short-circuits the request with
that response before the procedure runs. Use it when you want a
response other than a blank 401 — most commonly, to distinguish
*invalid* tokens from *valid but banned* users:

```rust,ignore
use std::sync::Arc;
use teleport::{AppError, TeleportRouter};

let app = TeleportRouter::new()
    .state(Arc::clone(&state))
    .try_auth("session", |token: String, state: Arc<AppState>| async move {
        match state.validate_session(&token).await {
            Some(user) if user.banned => {
                // Authenticated, but we're refusing to serve them.
                Err(AppError::<()>::Forbidden)
            }
            Some(user) => Ok(user),
            None => {
                // Bad or expired token.
                Err(AppError::<()>::Unauthorized)
            }
        }
    })
    .mount();
```

You can also return `AppError::Detail { detail: T }` from `try_auth` to
carry structured information (e.g. `{ "locked_until": "2026-01-01" }`)
that the client can render directly. The `E` parameter of the
middleware is a fresh generic — it does not have to match any
particular procedure's error type.

> ⚠️ **Typed `Detail` from `try_auth` is not type-safe on the TypeScript
> side.** The auth middleware short-circuits with an `AppError<E>`
> response where `E` is whatever you chose at `.try_auth(...)` time,
> but each procedure's generated TS client narrows the error branch as
> `AppError<ProcError>` — where `ProcError` is that procedure's own
> detail type. If you return a typed auth `Detail`, the TS client will
> silently attempt to decode it as the procedure's detail type, and
> may produce `undefined` fields at runtime even though `tsc` is happy.
>
> **Recommended**: only use `Err(AppError::<()>::Unauthorized)` or
> `Err(AppError::<()>::Forbidden)` from `try_auth` validators. Either
> of those serialises to `{"type":"Unauthorized"}` or
> `{"type":"Forbidden"}` — the variant tag is procedure-independent
> and narrows safely in every client-side match. If you need
> structured auth rejection data, put it in a cookie or a separate
> `/auth/status` procedure that every client polls after a 401/403.

If *no* token is present in the request at all, `try_auth` still passes
through without calling the validator. The "no auth attempted" case is
left to downstream extractors, which will return `Unauthorized` if the
procedure requires a user. This matches `auth`'s semantics.

Choose `auth` if:

- Most procedures do not require authentication, OR
- You are happy with a plain `401 Unauthorized` when a token is
  invalid.

Choose `try_auth` if:

- You need to return `403 Forbidden` for banned/locked accounts, OR
- You need to return a typed `Detail` payload from the auth layer
  (e.g. `{"type":"Detail","detail":{"reason":"banned"}}`).

Either way, procedures still opt in to authentication by taking an
`AuthedUser` or a `#[auth]`-annotated parameter. The router builder
only installs the middleware; it is not a global gate. See
`docs/security.md` for the full rationale.

## Debugging

### The manifest endpoint

With the `debug-manifest` feature enabled (or `.manifest(true)` on the
builder) teleport-rs mounts `GET /rpc/__manifest`. The response is a
JSON dump of every registered procedure:

```json
{
  "procedures": {
    "users.getUser": { "method": "GET", "path": "/rpc/users.getUser" },
    "auth.login":    { "method": "POST", "path": "/rpc/auth.login" }
  }
}
```

Use it to confirm a procedure is actually registered and to check the
route it was mounted under. Turn it off in production (omit the feature
and don't call `.manifest(true)`). See `docs/feature-flags.md`.

### Response serialization failures

`AppError::into_response` serializes the error with `serde_json`. If
that somehow fails (for example because a custom `Detail` type has a
buggy `Serialize` impl), the handler falls back to a static body:

```json
{"type":"Internal","message":"error serialization failed"}
```

A line is logged to stderr: `teleport-rs: failed to serialize AppError: ...`.
The response status is still derived from the original variant, so a
`NotFound` with a broken detail type still returns `404` — just with a
different body. If you see this line in logs, fix the `Serialize` impl
on your detail type.

### Panic recovery

By default `TeleportRouter::mount()` wraps the router in
`tower_http::catch_panic::CatchPanicLayer`. A panic in any handler is
logged to stderr (`teleport-rs: handler panicked: ...`) and the client
receives a generic JSON `500`:

```json
{"ok":false,"error":"internal server error"}
```

The panic payload is *never* included in the response — it may contain
raw request fields, stack-allocated values, or secrets. If you want
panics to take down the process instead (for example, under a
supervisor that restarts on every crash), call `.no_catch_panic()` on
the builder. Panic recovery is a safety net, not a substitute for
returning a real `AppError::Internal` from your handlers.

## See also

- [`docs/getting-started.md`](getting-started.md) — overall tour
- [`docs/security.md`](security.md) — production checklist
- [`docs/feature-flags.md`](feature-flags.md) — `export` and `debug-manifest`
- `crates/teleport/src/error.rs` — the enum and its `IntoResponse` impl
- `packages/client/src/result.ts` — TypeScript type guards
