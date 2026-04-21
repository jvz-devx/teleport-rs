# Contributing

## Toolchain

- **Rust**: stable. MSRV is **1.93** (constrained by `specta 2.0.0-rc.24`
  which uses `fmt::from_fn`). CI enforces this via a dedicated job.
  A `flake.nix` is provided for Nix users — `nix develop` drops you
  into a shell with Rust 1.93 + Node/npm + `cargo-deny` pinned.
- **Node/npm**: npm workspaces drive the JavaScript side. TypeScript and
  TS test execution come from local `typescript` / `tsx` devDependencies.

## First-time setup

```bash
# Rust
cargo build

# JS packages (installs typescript, tsx, qs, and workspace links)
npm install
```

## Running the full check suite

```bash
# Formatting
cargo fmt --all --check

# Lints (must pass with -D warnings)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests — the snapshot test hard-fails if `npm install` wasn't run,
# which is exactly what we want. Run it once and forget.
cargo test --workspace --all-features

# Rustdoc (warnings are errors)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

# TypeScript
npm exec tsc --noEmit -p packages/client/tsconfig.json
npm exec tsc --noEmit -p packages/vite/tsconfig.json
npm run test -w @teleport-rs/client
npm run test -w @teleport-rs/vite

# Supply-chain & licenses
cargo install cargo-deny --locked  # first time only
cargo deny --all-features check
```

CI runs all of the above on every push and pull request. See
`.github/workflows/ci.yml`.

## Test suite overview

- **Compile-fail tests** (`crates/teleport/tests/compile-fail/`) — use
  `trybuild` to verify that invalid `#[remote]` usage produces clear
  compiler errors.
- **HTTP integration tests** (`crates/teleport/tests/http.rs`) — full
  request/response round trips including auth, extractors, safety
  layers, and `try_auth` short-circuiting.
- **Extractor error paths** (`crates/teleport/tests/extractors.rs`) —
  malformed JSON, invalid form, garbage query.
- **Snapshot tests** (`crates/teleport-build/tests/snapshots.rs`) —
  both text snapshots via `insta` and a semantic `tsc --noEmit` step
  against the real `@teleport-rs/client` types (auto-read from
  `packages/client/src/types.ts` on every run, with a small hardcoded
  `TeleportClient` stub for generated-client checks). `tsc` is a hard
  prerequisite — if it's missing the test panics with a
  "run `npm install`" message.
- **TypeScript tests** (`packages/client/src/__tests__/`) — unit tests
  for the runtime helpers (result types, error handling).

## Code style

- Rust edition 2024, MSRV 1.93.
- Strict clippy — all warnings are errors in CI.
- `#![warn(missing_docs)]` is enforced on the `teleport` crate.
- No `unwrap()` / `expect()` in non-test library code.
- Integration / test files may scope-allow `clippy::expect_used`,
  `clippy::panic`, and `clippy::unused_async` as needed.

## Pull requests

1. Fork the repo.
2. Create a feature branch off `master`.
3. Make your changes — keep the diff focused; no drive-by refactors.
4. Run the full check suite above and confirm everything is green.
5. Open a PR against `master`. CI must be green before merge.
