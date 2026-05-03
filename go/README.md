# Teleport Go

The Go implementation is the third native host stack in this repo after Rust and `.NET`.

Module path:

```bash
go get github.com/jvz-devx/teleport-rs/go
```

Current shape:

- `teleport/`
  - portable contract types
  - result/error helpers
  - explicit procedure declarations
- `teleporthttp/`
  - `net/http` runtime with explicit registration
  - auth hook
  - manifest endpoint
- `examples/demo/`
  - a small Go demo backend that exports `teleport.contract.json`

This is intentionally not a port of Rust internals. Go does not get proc macros or implicit discovery. The authoring model is explicit registration against the shared contract boundary.

Preferred authoring style:

```go
teleport.QueryWithErrorFor[GetUserById, User, GetUserErrorDetail]("users", "getUser").
	Doc("Fetch a single user by ID.").
	Handle(func(ctx teleport.RequestContext, input GetUserById) teleport.Result[User] {
		user := state.user(input.ID)
		if user == nil {
			return teleport.Fail[User](
				teleport.DetailError(GetUserErrorDetail{UserNotFound: true}),
			)
		}
		return teleport.Ok(*user)
	})
```

Use `QueryFor`, `CommandFor`, or `FormFor` when the procedure has no typed detail error, and add `.RequireAuth()` or `.OptionalAuth()` when needed. The older `Query`, `Command`, and `Form` functions remain available for compact one-line registration.

1.0 status:

- query and form decoding now matches the shared parity cases covered in Rust and `.NET`, including nested bracket notation, indexed and append arrays, repeated keys, optional fields, and structured `400` failures for malformed input
- named contract types are still added explicitly to the router for now
- the TypeScript generation path is still shared through `teleport-cli`
- Go is a native host implementation against the shared contract, not a wrapper around Rust internals

Local commands:

```bash
CGO_ENABLED=0 go build ./...
CGO_ENABLED=0 go test ./...
CGO_ENABLED=0 go vet ./...
CGO_ENABLED=0 go run ./examples/demo --export-only
CGO_ENABLED=0 go run ./examples/demo
```

The current demo writes `teleport.contract.json` into `go/examples/demo/`.
From the repo root, use `npm run demo:export:go` to export the Go demo contract and regenerate TypeScript bindings through the shared `teleport-cli` path. Use `npm run contracts:parity` after exporting Rust, `.NET`, and Go contracts to confirm the demo contract shape still matches across platforms.
