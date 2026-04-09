# Feature Flags

The `teleport` crate has a small number of Cargo features that gate
development-only functionality. All features are off by default so a
release build of your server carries nothing it does not need.

## The matrix

| Feature          | Default | Purpose |
|------------------|---------|---------|
| `export`         | off     | Enables `TeleportRouter::export` / `.export_ts(...)` to generate TypeScript bindings. Pulls in `teleport-build` and its `specta-typescript` dependency. Use in dev, strip in prod. |
| `debug-manifest` | off     | Mounts `GET /rpc/__manifest` by default (without having to call `.manifest(true)`). Useful when sharing a running server with a frontend developer or writing integration tests. |

Neither feature changes the runtime behaviour of your procedures.
`export` only adds a build-time helper; `debug-manifest` only flips
the default value of the `.manifest()` builder method.

## Recommended configurations

### Development

```toml
[dependencies]
teleport = { version = "0.1", features = ["export", "debug-manifest"] }
```

This gives you `TeleportRouter::export(...)` for writing TypeScript
bindings and a live `/rpc/__manifest` for exploration during
development.

### Production

```toml
[dependencies]
teleport = { version = "0.1", default-features = false }
```

No TypeScript exporter compiled in, no manifest endpoint. The resulting
binary is smaller and has a smaller attack surface.

If you need the manifest in a single non-dev environment (say, a staging
server running integration tests), don't add the feature — call
`.manifest(true)` on the builder instead:

```rust,ignore
let app = TeleportRouter::new()
    .state(state)
    .manifest(cfg!(debug_assertions) || std::env::var("EXPOSE_MANIFEST").is_ok())
    .mount();
```

`.manifest(true)` always works regardless of whether the
`debug-manifest` feature is enabled — the feature only controls the
*default*.

### Dev/prod split with `cfg`

A common pattern is to gate the export call behind `debug_assertions`
or a feature of your own:

```rust,ignore
#[cfg(feature = "export")]
TeleportRouter::<AppState>::export(&ExportConfig::new("frontend/src/lib/api/generated"))
    .expect("failed to export TS bindings");
```

or simply run a separate `cargo run --features export` target in your
dev loop. Because `teleport-build` is an optional dependency, omitting
the feature completely removes it from your dependency graph.

## Why two features instead of one?

`export` pulls in `teleport-build`, `specta-typescript`, and their
transitive deps. That is a lot of compile time to pay for a feature you
only need in development. Splitting it from `debug-manifest` means you
can keep the manifest endpoint enabled (one conditional compilation of
a tiny `build_manifest()` function, no extra deps) without pulling in
the whole TypeScript exporter.

## See also

- [`docs/getting-started.md`](getting-started.md) — first steps
- [`docs/security.md`](security.md) — production hardening checklist
- `crates/teleport/Cargo.toml` — canonical list of features
