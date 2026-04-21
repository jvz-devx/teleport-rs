# @teleport-rs/client

TypeScript runtime for [teleport-rs](https://github.com/refactor-goblin/teleport-rs),
a Rust-to-TypeScript RPC framework.

This package is the small, framework-agnostic HTTP client consumed by
the code that `teleport-rs` generates from your Rust `#[remote]`
procedures. It ships type guards, error classes, and a thin `fetch`
wrapper — no React, no Svelte, no bundler magic.

## Install

```bash
npm install @teleport-rs/client
```

Usually installed alongside `@teleport-rs/vite` in dev:

```bash
npm install -D @teleport-rs/vite
```

## Usage

Create a client once at the entry point of your frontend:

```typescript
import { createClient } from "@teleport-rs/client";
import { bindClient } from "./generated/client";

const client = createClient({
  baseUrl: "/api",
  credentials: "include", // send the session cookie
});

export const { users } = bindClient(client);
```

Then use the bound generated client:

```typescript
import { isAppError, isTransportError } from "@teleport-rs/client";
import { users } from "./api";

const result = await users.getUser({ id: "123" });

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
  const user = rpcUnwrap(await users.getUser({ id: "123" }));
  return user;
} catch (err) {
  if (err instanceof TeleportError && err.is("NotFound")) {
    return null;
  }
  throw err;
}
```

`TeleportError.is(...)` only accepts known teleport app-error variants.
If you need to compare against an arbitrary string, read `err.appError.type`
directly instead of calling `.is(...)`.

## Exports

- `createClient(config)` — instance client factory; preferred for app wiring.
- `isAppError`, `isTransportError` — discriminated-union type guards.
- `rpcUnwrap`, `mapError` — ergonomic helpers for throwing vs. branching.
- `TeleportError`, `TransportFailure` — exception classes preserving the
  typed error payload.
- Types: `AppError`, `TransportError`, `RpcResult`, `HttpMethod`,
  `TeleportClient`, `RpcConfig`.

The client only classifies a non-OK response as a typed `AppError` when
the JSON matches the teleport error envelope and the HTTP status matches
the documented variant mapping. Malformed or mismatched error payloads
fall back to `TransportError`.

See [the main repo](https://github.com/refactor-goblin/teleport-rs) for
a full walkthrough, the Rust side of the story, and the generated
types your project will consume.
