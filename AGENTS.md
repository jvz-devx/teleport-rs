# AGENTS.md

Guidance for coding agents working in this repository.

## Project Shape

`teleport-rs` is a multi-language native backend project with one shared TypeScript generation path.

- Rust lives under `crates/` and is the reference implementation.
- `.NET` lives under `dotnet/` and targets ASP.NET Core.
- Go lives under `go/` and targets `net/http`.
- TypeScript runtime packages live under `packages/`.
- The demo frontend lives under `examples/demo/frontend`.
- The shared portable boundary is `teleport.contract/v1`, exported as `teleport.contract.json`.

Do not treat Rust internals as the cross-language API. Rust proc macros, `inventory`, Axum, and Specta are implementation details. `.NET` and Go should stay idiomatic in their own host stacks while matching the shared contract and wire behavior.

## Editing Rules

- Preserve existing user changes in the working tree. Do not revert unrelated files.
- Prefer `rg`/`rg --files` for repo search.
- Keep docs and examples aligned with all three implementations when changing contract behavior.
- Do not use `paseo`.
- Avoid committing generated build output from `target/`, `bin/`, `obj/`, or `node_modules/`.
- If a contract feature changes, update Rust, `.NET`, Go, generated TypeScript behavior, and parity coverage together.

## Important Commands

Rust:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps
./scripts/package-publishable-crates.sh --allow-dirty
```

TypeScript:

```bash
npm install
npm run build:js
npm exec tsc --noEmit -p packages/client/tsconfig.json
npm exec tsc --noEmit -p packages/vite/tsconfig.json
npm run check -w examples/demo/frontend
npm run build -w examples/demo/frontend
```

`.NET`:

```bash
npm run dotnet:build
npm run dotnet:test
npm run demo:export:dotnet
```

Go:

```bash
npm run go:build
npm run go:test
npm run demo:export:go
```

Parity:

```bash
npm run demo:export
npm run demo:export:dotnet
npm run demo:export:go
npm run contracts:parity
```

Use `CGO_ENABLED=0` for Go commands in this repo. The CI jobs do this, and local environments without `gcc` may fail plain `go test ./...`.

## Platform Syntax

Keep authoring syntax native per language:

- Rust: `#[remote(query)]`, `#[remote(command)]`, `#[remote(form)]`, `#[teleport_type]`.
- `.NET`: `[TeleportModule]`, `[TeleportQuery]`, `[TeleportCommand]`, `[TeleportForm]`, `[TeleportAuth]`.
- Go: `teleport.QueryFor`, `teleport.QueryWithErrorFor`, `teleport.CommandFor`, `teleport.FormFor` builders.

The invariant is not identical syntax. The invariant is identical exported contract shape, route shape, request decoding behavior, auth semantics, error envelopes, and generated TypeScript client behavior.

## CI Expectations

The GitHub workflow validates:

- Rust fmt, clippy, tests, docs, MSRV, package staging, and cargo-deny.
- TypeScript package builds/tests and npm dry-run packing.
- `.NET` restore/build/test/pack.
- Go test/vet/build via npm scripts and direct CI commands.
- Demo frontend check/build using bindings generated from Rust, `.NET`, and Go.
- Contract parity across Rust, `.NET`, and Go demos.

Passing tests are not enough if a new feature is not represented in the contract parity fixtures. Add demo or fixture coverage before considering parity complete.
