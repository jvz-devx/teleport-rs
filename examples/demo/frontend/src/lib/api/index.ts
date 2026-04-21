// Re-export generated types and errors from TeleportRouter::export().
// The `generated/` directory is populated at server startup via TeleportRouter::export().
export * from './generated/types';
export * from './generated/errors';
export { api } from './config';

// Re-export client utilities for convenience.
export { createClient, type RpcConfig, type RpcResult } from '@teleport-rs/client';
export { isTransportError, isAppError, rpcUnwrap, mapError } from '@teleport-rs/client';
