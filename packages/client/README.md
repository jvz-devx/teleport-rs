# @teleport-rs/client

TypeScript runtime for [teleport-rs](https://github.com/refactor-goblin/teleport-rs),
a Rust-to-TypeScript RPC framework.

This package is the small, framework-agnostic HTTP client consumed by
the code that `teleport-rs` generates from your Rust `#[remote]`
procedures. It ships type guards, error classes, and a thin `fetch`
wrapper — no React, no Svelte, no bundler magic.

## Install

```bash
bun add @teleport-rs/client         # or: npm install / pnpm add
```

Usually installed alongside `@teleport-rs/vite` in dev:

```bash
bun add -D @teleport-rs/vite
```

## Usage

Configure once at the entry point of your frontend:

```typescript
import { configure } from "@teleport-rs/client";

configure({
  baseUrl: "/api",
  credentials: "include", // send the session cookie
});
```

Then import from your generated client module:

```typescript
import { users } from "./generated/client";
import { isAppError, isTransportError } from "@teleport-rs/client";

const result = await users.getUser("123");

if (isTransportError(result)) {
  console.error("network problem", result.transport);
} else if (isAppError(result)) {
  console.error("server said no", result.error);
} else {
  console.log(result.data.name); // fully typed
}
```

For contexts where throwing is fine (e.g. SvelteKit remote functions),
use `rpcUnwrap`:

```typescript
import { rpcUnwrap, TeleportError } from "@teleport-rs/client";

try {
  const user = rpcUnwrap(await users.getUser("123"));
  return user;
} catch (err) {
  if (err instanceof TeleportError && err.is("NotFound")) {
    return null;
  }
  throw err;
}
```

## Exports

- `createClient(config)` — low-level client factory used by generated code.
- `configure(config)` / `getConfig()` — global configuration.
- `rpc` — the call function used inside generated modules.
- `isAppError`, `isTransportError` — discriminated-union type guards.
- `rpcUnwrap`, `mapError` — ergonomic helpers for throwing vs. branching.
- `TeleportError`, `TransportFailure` — exception classes preserving the
  typed error payload.
- Types: `AppError`, `TransportError`, `RpcResult`, `HttpMethod`,
  `TeleportClient`, `RpcConfig`.

See [the main repo](https://github.com/refactor-goblin/teleport-rs) for
a full walkthrough, the Rust side of the story, and the generated
types your project will consume.
