# teleport-rs — Lessons Learned

Post-implementation retrospective after completing Phases 1-6. Documents what worked, what didn't, and what needs to change before teleport-rs is usable by others.

---

## 1. Export Binary Ceremony

### What we designed

Separate `teleport-build` crate that accepts `ProcedureInfo` structs and generates TypeScript. This avoids a circular dependency: `teleport` depends on `teleport-macros`, and `teleport-build` can't depend on `teleport` without creating a cycle.

### What happened

Every project needs ~65 lines of boilerplate in `src/bin/export.rs`:

```rust
// This is what every user has to write today:
const fn convert_method(method: teleport::HttpMethod) -> BuildHttpMethod {
    match method {
        teleport::HttpMethod::Get => BuildHttpMethod::Get,
        teleport::HttpMethod::Post => BuildHttpMethod::Post,
    }
}

fn collect_procedures() -> (Vec<ProcedureInfo>, ResolvedTypes) {
    let mut types = Types::default();
    let mut procedures = Vec::new();
    for reg in inventory::iter::<ProcedureRegistration> {
        let info = ProcedureInfo {
            name: reg.name(),
            method: convert_method(reg.method),
            path: reg.path(),
            doc: reg.doc.to_owned(),
            input_type: (reg.input_type)(&mut types),
            output_type: (reg.output_type)(&mut types),
            error_type: (reg.error_type)(&mut types),
        };
        procedures.push(info);
    }
    let resolved = ResolvedTypes::from_resolved_types(types);
    (procedures, resolved)
}
```

This is not DX — it's plumbing. The user shouldn't need to know about `Types`, `ResolvedTypes`, `ProcedureInfo`, or the `HttpMethod` conversion.

### Root cause

The `teleport` ↔ `teleport-build` split created two parallel type hierarchies:
- `teleport::HttpMethod` vs `teleport_build::HttpMethod` (identical enums)
- `teleport::ProcedureRegistration` vs `teleport_build::ProcedureInfo` (same data, different shapes)

### Proposed fix

**Option A:** Create `teleport-core` crate with shared types. Both `teleport` and `teleport-build` depend on it. The export binary becomes:
```rust
fn main() {
    teleport_build::export_from_inventory(Config { output_dir: "...".into(), ..Default::default() });
}
```

**Option B:** Have `teleport-build` depend on `teleport` directly. The circular dependency only exists through `teleport-macros` — but `teleport-build` doesn't need macros. Check if Cargo allows this (it should, since the cycle would be `teleport -> teleport-macros`, `teleport-build -> teleport`, with no actual cycle).

Option B is simpler if it works.

---

## 2. Repetitive TypeScript Error Handling

### What we designed

`RpcResult<T, E>` discriminated union — never throws, forces exhaustive handling:

```typescript
type RpcResult<T, E> =
  | { ok: true; data: T }
  | { ok: false; error: AppError<E> }
  | { ok: false; transport: TransportError };
```

### What happened

Every function in `data.remote.ts` has the same 5-6 lines of error unwrapping:

```typescript
const result = await users.getUser(id);
if (!result.ok) {
    if (isTransportError(result)) throw new Error(result.transport.message);
    if (result.error.type === "NotFound") throw new Error("User not found");
    throw new Error(result.error.type);
}
return result.data;
```

The `unwrap()` helper exists but flattens all errors into a generic `Error` — the typed detail information is lost.

### Root cause

The Result pattern is correct for the *client library*, but in the *SvelteKit consumption layer* you almost always want to throw. The bridge between "typed result" and "throw for SvelteKit" is missing.

### Proposed fix

Add convenience helpers to `@teleport-rs/client`:

```typescript
// Throws a TeleportError (extends Error) that preserves the full typed error
export function rpcUnwrap<T, E>(result: RpcResult<T, E>): T { ... }

// Transform errors while keeping the result pattern
export function mapError<T, E, R>(
    result: RpcResult<T, E>,
    handler: (error: AppError<E>) => R
): T | R { ... }
```

And a `TeleportError` class that carries the original `AppError<E>` so you can catch and inspect it downstream.

---

## 3. Double Validation (Rust serde + TypeScript Zod)

### What we designed

Defense in depth: Zod validates on the SvelteKit BFF for instant UX feedback, serde validates in Rust for security (decision 13).

### What happened

Every type change requires editing both languages. The Zod schemas in `data.remote.ts` are hand-written approximations of the Rust types — they can drift silently.

```typescript
// This must match Rust's CreatePostRequest exactly, but nothing enforces it
z.object({ title: z.string().min(1), body: z.string().min(1) })
```

### Status

Accepted tech debt. Decision 14 deferred auto-generation until `specta-zod` is production-ready. The friction is real but the alternative (no client-side validation) is worse for UX.

### Future fix

When `specta-zod` stabilizes, generate Zod schemas alongside TypeScript types. Until then, document the pattern clearly and accept the duplication.

---

## 4. SvelteKit Remote Functions Dependency

### What we designed

teleport-rs as a bridge specifically for SvelteKit remote functions (`$app/server`).

### What happened

The `$app/server` API is experimental and behind a feature flag. The example code imports from it:

```typescript
import { query, command, form } from '$app/server'; // experimental!
```

If SvelteKit changes this API (or drops it), every integration pattern we documented breaks.

### Realization

The generated TypeScript client (`client.ts`) is actually framework-agnostic — it's just typed functions that call `fetch`. The SvelteKit-specific layer is only `data.remote.ts`, which is hand-written. teleport-rs's value proposition (Rust → typed TS client) doesn't depend on SvelteKit at all.

### Proposed fix

Reposition teleport-rs as **framework-agnostic** with SvelteKit as one example:
- The core value is: write Rust, get a typed TypeScript client
- Show usage with SvelteKit, Next.js, plain fetch, React Query
- Don't couple the docs or naming to SvelteKit specifically

---

## 5. Two Identical HttpMethod Enums

### What happened

`teleport::procedure::HttpMethod` and `teleport_build::HttpMethod` are the same enum, duplicated to avoid a dependency cycle. Every export binary needs a `convert_method()` function to bridge them.

### Proposed fix

Resolved by fixing issue #1 (export binary simplification). Either a shared crate or direct dependency eliminates the duplication.

---

## Summary: What's Worth Keeping vs What Needs Work

| Area | Verdict | Action |
|---|---|---|
| `#[remote]` proc macro | Genuinely good DX | Keep |
| `AppError<T>` pattern | Clean, well-designed | Keep |
| `TeleportRouter` builder | Works well | Keep |
| Auth middleware | Flexible, clean API | Keep |
| `RpcResult<T, E>` type | Correct but needs helpers | Add convenience layer |
| Export binary | Too much boilerplate | Simplify to one-liner |
| Type generation (specta) | Works but RC version | Monitor specta v2 stable |
| SvelteKit coupling | Unnecessary constraint | Reposition as framework-agnostic |
| Zod duplication | Accepted debt | Wait for specta-zod |
