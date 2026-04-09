# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-04-09

### Fixed

- **`Vec<T>` no longer leaks as a named import in generated `client.ts`.**
  Previously, any procedure returning `Vec<Todo>` generated
  `import type { ..., Vec } from "./types";` followed by
  `Promise<RpcResult<Vec<Todo>, null>>`, and `types.ts` never exported
  `Vec` — so `tsc --noEmit` failed with `TS2305`. `teleport-build` now
  translates `Vec<T>` to the inline `T[]` construct and filters
  stdlib-wrapper names out of the import collection.
- **`String` no longer leaks as a named import.** Same class of bug as
  `Vec<T>` — specta registers `std::string::String` as a named type,
  and teleport-build's import filter only checked lowercase `"string"`.
  Now translated to the TypeScript `string` primitive inline.
- **`HashMap` / `BTreeMap` / `HashSet` / `BTreeSet` / `VecDeque` /
  `LinkedList` / `BinaryHeap`** all get the same treatment — translated
  to `Record<K, V>` / `T[]` inline, never leaked into imports.
- **64-bit integer primitives (`i64` / `u64` / `i128` / `u128` /
  `isize` / `usize`) no longer panic at export.** `specta-typescript`
  0.0.11 refuses these types because they would lose precision as JS
  `number`; `teleport-build` now walks the resolved type collection
  before handing it to specta and rewrites every 64-bit primitive to
  `str`, producing the TypeScript `string` type. The JSON wire format
  remains `"123"` (string) for round-trip safety.
- **Error messages from type export failures now include a
  "while generating X" breadcrumb** so users can triage the previous
  empty-type-name specta error (`"Attempted to export \"\" but Specta
  forbids ..."`).

### Known Issues

- **Enum `AppError::Detail<T>` with struct or tuple variants renders
  broken TypeScript.** Given
  `enum E { A, B { reason: String }, C { reason: String } }`, the
  generated TS collapses `B` and `C` to the same `{ reason: string }`
  shape, drops the outer variant tag, and leaves `tsc` unable to catch
  the resulting runtime bug (`detail.reason` is `undefined` at
  runtime). `#[serde(tag = "...")]` is silently ignored by
  `specta-typescript` 0.0.11 — the serde-level escape hatch does not
  apply.

  **Workaround**: use a flat struct with `bool` / `Option<String>`
  fields for error details. See
  [`docs/error-handling.md` §"Detail type constraints"](docs/error-handling.md)
  for the full explanation and code example. Unit-only enums (no
  fielded variants) render correctly as TypeScript string literal
  unions and are also safe.

  Regression tests for this upstream bug live at
  `crates/teleport-build/tests/data_types.rs` as `#[ignore]`-annotated
  tests. Run with `cargo test -p teleport-build --test data_types -- --ignored`
  to verify. They currently pass (asserting the broken shape). When
  `specta-typescript` upstream fixes the bug, they will fail — which
  is the signal to update the docs and un-ignore the tests.

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
- **33 new data-type regression tests** in
  `crates/teleport-build/tests/data_types.rs` covering every Rust
  primitive (`bool`, all signed/unsigned ints, floats, `String`),
  every container (`Vec`, `Option`, `HashMap`, `BTreeMap`, `HashSet`,
  `BTreeSet`, `VecDeque`, tuples), nested structs, newtype structs,
  unit-only enums, the three 0.1.1 codegen-bug regressions, and 2
  `#[ignore]`-annotated tests documenting the enum-detail upstream
  bug. Run with `cargo test -p teleport-build --test data_types`
  (add `-- --ignored` to exercise the documented-bug tests).

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
