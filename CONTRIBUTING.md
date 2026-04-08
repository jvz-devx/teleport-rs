# Contributing

## Building

```bash
# Rust
cargo build

# npm packages
cd packages/client && npm install
cd packages/vite && npm install
```

## Testing

```bash
cargo test
cargo clippy -- -D warnings
cd packages/client && npx tsc --noEmit
cd packages/vite && npx tsc --noEmit
```

## Code Style

- Rust edition 2024, MSRV 1.91
- Strict clippy — all warnings are errors
- No `unwrap()` in library code

## Pull Requests

1. Fork the repo
2. Create a feature branch
3. Make your changes
4. Run tests and clippy
5. Open a PR against `master`
