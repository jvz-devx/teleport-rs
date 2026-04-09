// Re-export generated types and client namespaces.
// The `generated/` directory is populated at server startup via TeleportRouter::export().
export * from './generated/types';
export * from './generated/errors';
export { auth, users, posts } from './generated/client';

// Re-export client utilities for convenience.
export { configure, type RpcConfig, type RpcResult } from '@teleport-rs/client';
export { isTransportError, isAppError, rpcUnwrap } from '@teleport-rs/client';
