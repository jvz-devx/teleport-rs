# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Infrastructure

- **CI overhaul**: 8-job GitHub Actions workflow (`fmt`, `clippy`,
  `test`, `rustdoc`, `msrv`, `typescript`, `cargo-deny`, `ci` summary).
  Every Rust check runs with `-D warnings`; `rustdoc` also denies
  warnings. Uses `Swatinem/rust-cache` for build caching. Branch
  protection should require the `ci` summary job.
- Migrated the JS side from npm to **bun**. The demo frontend's
  `workspace:*` protocol is now handled natively, and `bun install`
  hoists TypeScript into `node_modules/.bin/tsc` so the snapshot test's
  `tsc --noEmit` check works in CI with zero extra setup.
- Deterministic codegen: `teleport-build` now sorts procedures by name
  after collecting from inventory, making `client.ts`, `errors.ts`, and
  namespace groupings stable across rustc versions and link orders.
  (Caught by the new snapshot test.)
- `deny.toml` for `cargo-deny` — advisory database, permissive-only
  license allow-list, crate source restrictions, wildcard-path allow
  for internal workspace crates.
- `dependabot.yml` — weekly updates for `cargo` and `github-actions`
  with minor+patch grouping.

### Added

- `TeleportRouter::try_auth` — fallible auth validator variant that
  short-circuits the request with an `AppError<E>` response when the
  validator returns `Err`. Use for banned-user rejection, typed
  lockout payloads, or anywhere a plain `401` is insufficient.
- `TeleportRouter::body_limit(bytes)` and `TeleportRouter::no_body_limit()`
  builder methods, backed by a new `DEFAULT_BODY_LIMIT` constant
  (2 MiB). Controls both `tower_http::limit::RequestBodyLimitLayer`
  and axum's `DefaultBodyLimit` so `Json`/`Form`/`Bytes` extractors
  honour the same limit.
- `TeleportRouter::no_catch_panic()` builder method for opting out of
  the default panic-recovery layer (for example, under a supervisor
  that should restart on every panic).
- New documentation: `docs/security.md`, `docs/error-handling.md`,
  `docs/feature-flags.md`.
- Minimal starter example at `examples/starter/` — a single-file,
  ~50-line walkthrough of the full API for first-time users.
- `README.md` files for `@teleport-rs/client` and `@teleport-rs/vite`
  npm packages.
- `#![warn(missing_docs)]` on the `teleport` crate, with a crate-level
  `//!` doc comment.
- `teleport-build` snapshot tests now also run the generated TypeScript
  through `tsc --noEmit` against the real `@teleport-rs/client` types,
  catching semantic regressions (syntax errors, broken generics, missing
  fields) that a plain text snapshot would miss. The stub is auto-built
  from `packages/client/src/types.ts` on every run so it cannot drift;
  the one hardcoded piece (the `rpc` signature) is guarded by a sentinel
  that reads `packages/client/src/rpc.ts`. `tsc` is a hard prerequisite —
  the test panics with a "run `bun install`" message if it's missing.

### Changed

- Proc-macro error messages now point at the offending span and
  include actionable fix hints for the most common mistakes
  (missing `#[teleport_type]`, wrong `#[remote]` kind, etc.).
- The `examples/demo/` crate now uses an explicit `CorsLayer`
  configuration with an allow-list of origins, methods, headers, and
  credentials — no more `CorsLayer::permissive()`.

### Security

- **Default 2 MiB request body size limit** applied by
  `TeleportRouter::mount()`. Oversized requests are rejected with
  `413 Payload Too Large` before any handler runs. Override with
  `.body_limit(bytes)`, disable with the deliberately-loud
  `.no_body_limit()`.
- **Default panic recovery** via `tower_http::catch_panic::CatchPanicLayer`.
  A panicking handler returns a generic JSON `500` and logs the
  payload to stderr instead of taking down the process. Opt out with
  `.no_catch_panic()`. The panic payload is **never** included in
  the response body.
