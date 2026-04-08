// Re-export generated types and client namespaces.
// The `generated/` directory is populated by `cargo run --bin export`.
export * from './generated/types';
export * from './generated/errors';
export { auth, users, posts } from './generated/client';

// Re-export client utilities for convenience.
export { configure, type RpcConfig, type RpcResult } from '@teleport-rs/client';
export { isTransportError, isAppError, unwrap } from '@teleport-rs/client';
