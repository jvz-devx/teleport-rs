//! Snapshot tests for generated TypeScript output.
//!
//! These tests lock in the *exact* shape of the `client.ts`, `errors.ts`,
//! and `types.ts` files that `teleport-build` emits. Codegen regressions
//! (added whitespace, reordered imports, changed identifier cases, leaked
//! internals) fail loudly here instead of silently shipping broken clients.
//!
//! In addition to textual snapshots, the generated files are run through
//! `tsc --noEmit` with a stub `@teleport-rs/client` declaration so any
//! *semantic* TypeScript regressions (syntax errors, bad identifiers,
//! missing fields, wrong generics) fail here as well. `tsc` is a hard
//! prerequisite — if it isn't found, the test panics with a clear
//! "run `bun install`" message. Run `bun install` once from the repo
//! root (or enter the `nix develop` shell) and it Just Works.
//!
//! # Stub drift mitigation
//!
//! The `@teleport-rs/client` stub fed to tsc is **not** hand-maintained.
//! It is rebuilt from the real client source on every test run:
//!
//! - The type portion is read verbatim from
//!   `packages/client/src/types.ts`, so any rename, field addition, or
//!   discriminator change in the real client flows through automatically
//!   and will fail the snapshot's tsc step if the codegen is out of sync.
//! - The `rpc` function signature is hardcoded in `build_client_stub_dts`
//!   because `rpc.ts` has a body and can't be inlined into a `.d.ts`.
//!   It is guarded by `assert_real_rpc_matches_sentinel`, which reads
//!   `packages/client/src/rpc.ts` and fails loudly if the signature
//!   shape changes.
//!
//! If the sentinel fails, update the hardcoded declaration in
//! `build_client_stub_dts` *and* the `RPC_SIGNATURE_SENTINEL` constant
//! in the same PR.
//!
//! # How it works
//!
//! Integration tests only see `teleport-build`'s public API, and the
//! `ProcedureInfo` shape is `pub(crate)`. So instead of calling the
//! generator directly, we register fixture `ProcedureRegistration`s into
//! `teleport-core`'s inventory (which lives per-test-binary), then call
//! `export_from_inventory` to produce real output files into a `TempDir`,
//! and snapshot the contents.
//!
//! The fixtures are handcrafted to cover:
//!   - struct input + struct output + struct error (the common case)
//!   - unit input (`()`) for zero-argument procedures
//!   - unit error (`()` → TS `null`) — matches what the `#[remote]` macro
//!     emits for `AppError` without a type parameter
//!   - multiple namespaces, so namespace grouping is snapshotted

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use serde::{Deserialize, Serialize};
use specta::Type;
use teleport_build::{Config, export_from_inventory};
use teleport_core::{HttpMethod, ProcedureRegistration, ProcedureType};

// ---------------------------------------------------------------------------
// Fixture types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[allow(dead_code)]
struct SnapshotUser {
    id: String,
    name: String,
    email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[allow(dead_code)]
struct SnapshotUserId {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[allow(dead_code)]
struct SnapshotCreateUserInput {
    name: String,
    email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[allow(dead_code)]
struct SnapshotCreateUserError {
    field: String,
    reason: String,
}

// ---------------------------------------------------------------------------
// Fixture procedures — registered into this test binary's inventory.
// ---------------------------------------------------------------------------

/// Stub mount fn. `export_from_inventory` never calls this, so returning a
/// dummy `Box<dyn Any + Send>` is sufficient; it will never be downcast.
fn stub_mount() -> Box<dyn std::any::Any + Send> {
    Box::new(())
}

// `users.getUser` — query, struct input + struct output, no typed error.
inventory::submit! {
    ProcedureRegistration {
        module_path: "snapshot_tests::users",
        fn_name: "getUser",
        prefix: Some("users"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <SnapshotUserId as specta::Type>::definition(types),
        output_type: |types| <SnapshotUser as specta::Type>::definition(types),
        error_type: |types| <() as specta::Type>::definition(types),
        doc: "Look up a user by their ID.",
        mount_fn: stub_mount,
    }
}

// `users.createUser` — command, struct input + struct output + typed error.
inventory::submit! {
    ProcedureRegistration {
        module_path: "snapshot_tests::users",
        fn_name: "createUser",
        prefix: Some("users"),
        method: HttpMethod::Post,
        procedure_type: ProcedureType::Command,
        input_type: |types| <SnapshotCreateUserInput as specta::Type>::definition(types),
        output_type: |types| <SnapshotUser as specta::Type>::definition(types),
        error_type: |types| <SnapshotCreateUserError as specta::Type>::definition(types),
        doc: "Create a new user account.",
        mount_fn: stub_mount,
    }
}

// `health.check` — query, no input, no output body (unit), no error.
inventory::submit! {
    ProcedureRegistration {
        module_path: "snapshot_tests::health",
        fn_name: "check",
        prefix: Some("health"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as specta::Type>::definition(types),
        output_type: |types| <() as specta::Type>::definition(types),
        error_type: |types| <() as specta::Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run `export_from_inventory` into a scratch directory. Returns the
/// `TempDir` (so the caller can keep it alive for follow-up checks like
/// `tsc --noEmit`) and the contents of every generated file as
/// `(relative_name, content)` pairs.
fn generate_into_tempdir() -> (tempfile::TempDir, Vec<(String, String)>) {
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    let out_dir = tmp.path().to_path_buf();

    let config = Config::new(out_dir.clone())
        .with_prefix("/rpc")
        .with_client_import("@teleport-rs/client");

    export_from_inventory(&config).expect("export_from_inventory failed");

    let mut files = Vec::new();
    for name in ["types.ts", "errors.ts", "client.ts", "index.ts"] {
        let path = out_dir.join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read generated {name}: {e}"));
        files.push((name.to_owned(), content));
    }
    (tmp, files)
}

// ---------------------------------------------------------------------------
// tsc --noEmit validation
// ---------------------------------------------------------------------------

/// Sentinel substring that must appear in `packages/client/src/rpc.ts`.
///
/// The `rpc` declaration in [`build_client_stub_dts`] is hardcoded — it
/// cannot be auto-read from `rpc.ts` because the real file has a function
/// body, which is invalid in a `.d.ts` context. To prevent silent drift
/// between the hardcoded stub signature and the real signature, every
/// test run asserts that `rpc.ts` still contains this exact substring.
///
/// If this sentinel disappears, the `rpc` signature has been refactored
/// — update both this constant and the hardcoded declaration in
/// `build_client_stub_dts` in the same commit.
const RPC_SIGNATURE_SENTINEL: &str = "export async function rpc<T, E>(";

/// Build the `.d.ts` stub that stands in for `@teleport-rs/client` when
/// type-checking the generated TS.
///
/// The type declarations are read verbatim from
/// `packages/client/src/types.ts` so they cannot drift. The `rpc`
/// function signature is hardcoded and guarded by
/// [`assert_real_rpc_matches_sentinel`].
fn build_client_stub_dts(repo_root: &std::path::Path) -> String {
    let types_ts = std::fs::read_to_string(repo_root.join("packages/client/src/types.ts"))
        .expect("read packages/client/src/types.ts");

    let mut stub = String::with_capacity(types_ts.len() + 256);
    stub.push_str("// Auto-built from packages/client/src/types.ts at test time.\n");
    stub.push_str(
        "// Drift from the real types.ts is impossible — this file is read fresh on every run.\n",
    );
    stub.push_str(
        "// The `rpc` signature below is guarded by assert_real_rpc_matches_sentinel().\n\n",
    );
    stub.push_str(&types_ts);
    stub.push_str(
        "\n// --- rpc function (hardcoded; sentinel-guarded) ---\n\
         export declare function rpc<T, E>(\n  \
           method: HttpMethod,\n  \
           path: string,\n  \
           input: unknown,\n\
         ): Promise<RpcResult<T, E>>;\n",
    );
    stub
}

/// Guard against drift in the hardcoded `rpc` portion of the stub.
///
/// If `rpc.ts` no longer contains the expected signature shape, the
/// hardcoded declaration in [`build_client_stub_dts`] is stale — update
/// both the stub and [`RPC_SIGNATURE_SENTINEL`] together.
fn assert_real_rpc_matches_sentinel(repo_root: &std::path::Path) {
    let rpc_ts = std::fs::read_to_string(repo_root.join("packages/client/src/rpc.ts"))
        .expect("read packages/client/src/rpc.ts");

    assert!(
        rpc_ts.contains(RPC_SIGNATURE_SENTINEL),
        "teleport-build snapshot test sentinel failed: \
         `packages/client/src/rpc.ts` no longer contains the string {RPC_SIGNATURE_SENTINEL:?}. \
         The `rpc` function signature has changed. Update the hardcoded \
         declaration in `build_client_stub_dts()` in this test file to match, \
         and update `RPC_SIGNATURE_SENTINEL` to the new shape.",
    );
}

/// Compute the absolute path of the repo root from this crate's
/// `CARGO_MANIFEST_DIR` (which is `<repo>/crates/teleport-build`).
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("CARGO_MANIFEST_DIR should have a grandparent (the repo root)")
        .to_path_buf()
}

/// `tsconfig.json` that points `@teleport-rs/client` at the stub `.d.ts`
/// and type-checks every generated file under strict settings.
const TSCONFIG_JSON: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ES2022",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": false,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "baseUrl": ".",
    "paths": {
      "@teleport-rs/client": ["./teleport_rs_client_stub.d.ts"]
    }
  },
  "include": ["client.ts", "errors.ts", "types.ts", "index.ts", "teleport_rs_client_stub.d.ts"]
}
"#;

/// Type-check every generated file in `out_dir` against the auto-built
/// `@teleport-rs/client` stub. Panics (failing the test) on:
///
/// - A drift in `rpc.ts` that defeats the sentinel.
/// - `tsc` not being installed (install `bun install` from the repo
///   root, or enter `nix develop`).
/// - Any type error reported by `tsc --noEmit`.
fn type_check_with_tsc(out_dir: &std::path::Path) {
    // Fail fast if the hardcoded `rpc` declaration in `build_client_stub_dts`
    // has drifted from the real `packages/client/src/rpc.ts`. This keeps
    // the single remaining piece of hand-synchronised stub honest.
    let root = repo_root();
    assert_real_rpc_matches_sentinel(&root);

    let stub = build_client_stub_dts(&root);
    std::fs::write(out_dir.join("teleport_rs_client_stub.d.ts"), stub).expect("write stub .d.ts");
    std::fs::write(out_dir.join("tsconfig.json"), TSCONFIG_JSON).expect("write tsconfig.json");

    let tsc_cmd = find_tsc().unwrap_or_else(|| {
        panic!(
            "\n\
             teleport-build snapshot test: could not find a `tsc` binary.\n\
             \n  \
               Checked: PATH, packages/client/node_modules/.bin/tsc, node_modules/.bin/tsc.\n\
             \n  \
               This test type-checks the generated TypeScript against the real\n  \
               `@teleport-rs/client` types. To fix, run from the repo root:\n\
             \n      \
                 bun install\n\
             \n  \
               (or enter the dev shell with `nix develop`, which provides bun).\n"
        );
    });

    let output = std::process::Command::new(&tsc_cmd)
        .arg("--noEmit")
        .arg("-p")
        .arg(out_dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn tsc at {tsc_cmd:?}: {e}"));

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "generated TypeScript failed tsc --noEmit:\n\
             --- stdout ---\n{stdout}\n\
             --- stderr ---\n{stderr}"
        );
    }
}

/// Locate a `tsc` executable. Checks, in order:
/// 1. `tsc` on `PATH`
/// 2. `packages/client/node_modules/.bin/tsc` relative to the repo root
/// 3. `node_modules/.bin/tsc` at the repo root (workspace-hoisted install)
fn find_tsc() -> Option<String> {
    if command_exists("tsc") {
        return Some("tsc".to_owned());
    }
    let root = repo_root();
    for candidate in [
        "packages/client/node_modules/.bin/tsc",
        "node_modules/.bin/tsc",
    ] {
        let path = root.join(candidate);
        if path.exists() {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    None
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Snapshot tests
// ---------------------------------------------------------------------------

#[test]
fn snapshot_generated_typescript() {
    let (tmp, files) = generate_into_tempdir();

    for (name, content) in &files {
        // Sanity: no absolute filesystem paths leaked in (TempDir paths
        // would change every run and poison the snapshot).
        assert!(
            !content.contains("/tmp/"),
            "generated {name} leaked a tempdir path: {content}",
        );
        // Sanity: no Debug output / inventory internals leaked.
        assert!(
            !content.contains("ProcedureRegistration"),
            "generated {name} leaked a Rust internal name: {content}",
        );
    }

    // Snap each file as a plaintext snapshot with a stable name. We pass
    // the content via `assert_snapshot!(name, content)` so the snapshot
    // file path is deterministic: `tests/snapshots/snapshots__<name>.snap`.
    for (name, content) in &files {
        let snap_name = format!("generated_{}", name.replace('.', "_"));
        insta::assert_snapshot!(snap_name, content);
    }

    // Semantic check: run the generated TS through `tsc --noEmit`.
    // This catches regressions the text snapshot can't — syntax errors,
    // broken generics, missing fields, wrong import paths. `tsc` is a
    // hard prerequisite; if it's missing the test panics with a clear
    // "run bun install" message. See `type_check_with_tsc`.
    type_check_with_tsc(tmp.path());
}
