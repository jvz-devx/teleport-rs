# Security

A short production checklist for teleport-rs servers. The defaults aim to be
safe for production. The escape hatches are deliberately loud.

## Request body size limit

`TeleportRouter::mount()` applies a 2 MiB request body size limit by default
via `tower_http::limit::RequestBodyLimitLayer`. Requests with bodies larger
than this are rejected with `413 Payload Too Large` before any handler runs.

```rust,ignore
// Default: 2 MiB
TeleportRouter::new().state(state).mount();

// Raise the limit (e.g. for an upload procedure):
TeleportRouter::new().state(state).body_limit(10 * 1024 * 1024).mount();

// Disable entirely (only when an upstream proxy enforces a limit):
TeleportRouter::new().state(state).no_body_limit().mount();
```

The `no_body_limit()` name is intentionally noisy so reviewers notice it.

## Panic recovery

By default, every router built with `mount()` is wrapped in
`tower_http::catch_panic::CatchPanicLayer`. If a procedure handler panics,
teleport-rs returns a generic JSON 500 instead of crashing the process:

```json
{"ok":false,"error":"internal server error"}
```

The panic payload is **never** included in the response body — only logged
to stderr (`teleport-rs: handler panicked: ...`). To opt out (e.g. when
running under a supervisor that should restart on every panic), call
`.no_catch_panic()` on the router builder.

## CORS

Examples and the demo crate use an explicit CORS configuration. **Never**
use `tower_http::cors::CorsLayer::permissive()` in production — it allows
any origin, any method, and any header, including credentials, which is
unsafe for any server holding session cookies.

The demo's starting point:

```rust,ignore
use tower_http::cors::CorsLayer;

CorsLayer::new()
    .allow_origin("https://app.example.com".parse::<http::HeaderValue>().unwrap())
    .allow_methods([http::Method::GET, http::Method::POST])
    .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION])
    .allow_credentials(true);
```

Adjust `allow_origin` to match your real frontend.

## Auth is opt-in

Routes without an `AuthedUser` parameter or a `#[auth]`-annotated extractor
do **not** require authentication. teleport-rs treats authentication as a
per-procedure feature, not a global gate. This is deliberate — it makes
public endpoints (login, healthchecks, public catalogues) trivial to express,
and it makes auth requirements visible at the procedure signature.

Audit each procedure's signature and confirm it asks for the user value it
needs.

## HTTPS termination

teleport-rs does not handle TLS. Run it behind a reverse proxy or load
balancer (nginx, Caddy, Cloudflare, an AWS ALB, etc.) that terminates HTTPS
and forwards plain HTTP on a private network. The body limit, panic
recovery, and auth middleware all assume the request reaching the Axum
router is trustworthy in transit.
