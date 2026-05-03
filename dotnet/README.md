# Teleport .NET

The `dotnet/` tree contains the first non-Rust Teleport backend implementation. It is a native ASP.NET Core host stack that exports the same `teleport.contract/v1` contract as the Rust and Go implementations.

Current packages:

- `src/Teleport.Net`
  - attributes, result/error types, and contract export
- `src/Teleport.Net.AspNetCore`
  - ASP.NET Core endpoint discovery, binding, auth handling, manifest endpoint, and HTTP runtime behavior
- `examples/Teleport.Net.Demo`
  - demo server that exports `teleport.contract.json` and serves the same frontend used by the Rust demo

## Public surface

`Teleport.Net`

- `[TeleportModule("users")]`
- `[TeleportQuery]`
- `[TeleportCommand]`
- `[TeleportForm]`
- `[TeleportName("getUser")]`
- `[TeleportAuth]`
- `TeleportResult<TOutput, TError>`
- `AppError<TDetail>`
- `TeleportContractExporter.Build(...)`
- `TeleportContractExporter.ExportJson(...)`

`Teleport.Net.AspNetCore`

- `services.AddTeleport()`
- `app.MapTeleportEndpoints(options => ...)`

## Intentional v1 constraints

- Procedures must be declared on `public static` classes marked with `[TeleportModule]`
- Procedure methods must be `public static`
- Return type must be `TeleportResult<TOutput, TError>` or `Task<TeleportResult<TOutput, TError>>`
- Query payloads must be struct/class wrappers, not bare primitive inputs
- Auth parameters are `ClaimsPrincipal` or `ClaimsPrincipal?`
- Dependency injection parameters use ASP.NET's existing `[FromServices]`

These are deliberate portability constraints. They keep the exported contract stable across Rust and `.NET`.
They also keep the `.NET` behavior aligned with Go and the shared TypeScript generator.

## Contract boundary

The language-neutral handoff is `teleport.contract.json`.

Typical flow:

1. The backend exports a `ContractBundle`
2. `teleport-cli generate-ts --input teleport.contract.json --output ...` generates TypeScript bindings
3. The frontend uses the generated client against the Rust, `.NET`, or Go backend

## Validation

Local commands used in the repo:

```bash
DOTNET_CLI_HOME=/tmp dotnet build dotnet/Teleport.Net.sln -c Release -maxcpucount:1 -nodeReuse:false
DOTNET_CLI_HOME=/tmp dotnet test dotnet/Teleport.Net.sln -c Release -maxcpucount:1 -nodeReuse:false
npm run demo:export:dotnet
npm run check -w examples/demo/frontend
npm run build -w examples/demo/frontend
```

## Status

The `.NET` implementation is the first stabilized non-Rust backend in this repo.

It is not a promise of permanent API freeze, but it is expected to stay aligned with:

- the shared contract schema
- the generated TypeScript client shape
- the Rust and Go demos' externally visible behavior for the overlapping feature set
