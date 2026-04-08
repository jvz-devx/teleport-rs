//! Integration test: full export pipeline.
//!
//! Defines procedures with `#[remote]`, collects them via `inventory`,
//! and verifies the generated TypeScript files using `export_from_inventory`.

#![allow(clippy::expect_used)]

use std::sync::Arc;

use teleport::{remote, teleport_type, AppError, TeleportRouter};
use teleport_build::{Config, Naming, NamespaceStyle};

// ---------------------------------------------------------------------------
// Test types
// ---------------------------------------------------------------------------

#[teleport_type]
pub struct User {
    pub id: String,
    pub name: String,
}

#[teleport_type]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
}

#[teleport_type]
pub struct GetUserError {
    pub not_found: bool,
}

// ---------------------------------------------------------------------------
// Test state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TestState;

// ---------------------------------------------------------------------------
// Test procedures
// ---------------------------------------------------------------------------

/// Fetch a single user by ID.
#[remote(query)]
async fn get_user(_ctx: &TestState, _input: User) -> Result<User, AppError<GetUserError>> {
    Ok(User {
        id: "1".into(),
        name: "Alice".into(),
    })
}

/// Create a new user account.
#[remote(command)]
async fn create_user(
    _ctx: &TestState,
    _input: CreateUserRequest,
) -> Result<User, AppError> {
    Ok(User {
        id: "2".into(),
        name: "Bob".into(),
    })
}

#[remote(query)]
async fn list_users(_ctx: &TestState) -> Result<Vec<User>, AppError> {
    Ok(vec![])
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_config(output_dir: std::path::PathBuf) -> Config {
    Config {
        output_dir,
        namespace_style: NamespaceStyle::default(),
        naming: Naming::default(),
        include_manifest: false,
        route_prefix: "/rpc".to_owned(),
        client_import_path: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_generates_ts_files() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let config = test_config(tmp.path().to_path_buf());

    teleport_build::export_from_inventory(&config)
        .expect("generation should succeed");

    let types_ts = std::fs::read_to_string(tmp.path().join("types.ts"))
        .expect("types.ts should exist");
    let errors_ts = std::fs::read_to_string(tmp.path().join("errors.ts"))
        .expect("errors.ts should exist");
    let client_ts = std::fs::read_to_string(tmp.path().join("client.ts"))
        .expect("client.ts should exist");
    let index_ts = std::fs::read_to_string(tmp.path().join("index.ts"))
        .expect("index.ts should exist");

    // types.ts should contain our registered structs.
    assert!(types_ts.contains("User"), "types.ts missing User:\n{types_ts}");
    assert!(
        types_ts.contains("CreateUserRequest"),
        "types.ts missing CreateUserRequest:\n{types_ts}"
    );
    assert!(
        types_ts.contains("GetUserError"),
        "types.ts missing GetUserError:\n{types_ts}"
    );

    // errors.ts should contain the framework error types.
    assert!(
        errors_ts.contains("AppError"),
        "errors.ts missing AppError:\n{errors_ts}"
    );
    assert!(
        errors_ts.contains("TransportError"),
        "errors.ts missing TransportError:\n{errors_ts}"
    );
    assert!(
        errors_ts.contains("RpcResult"),
        "errors.ts missing RpcResult:\n{errors_ts}"
    );

    // client.ts should contain namespace and procedure functions.
    assert!(
        client_ts.contains("getUser"),
        "client.ts missing getUser:\n{client_ts}"
    );
    assert!(
        client_ts.contains("createUser"),
        "client.ts missing createUser:\n{client_ts}"
    );
    assert!(
        client_ts.contains("listUsers"),
        "client.ts missing listUsers:\n{client_ts}"
    );
    assert!(
        client_ts.contains("rpc(\"GET\""),
        "client.ts missing GET rpc call:\n{client_ts}"
    );
    assert!(
        client_ts.contains("rpc(\"POST\""),
        "client.ts missing POST rpc call:\n{client_ts}"
    );

    // index.ts should re-export all modules.
    assert!(
        index_ts.contains("export * from \"./types\""),
        "index.ts missing types re-export:\n{index_ts}"
    );
    assert!(
        index_ts.contains("export * from \"./errors\""),
        "index.ts missing errors re-export:\n{index_ts}"
    );
    assert!(
        index_ts.contains("export * from \"./client\""),
        "index.ts missing client re-export:\n{index_ts}"
    );
}

#[test]
fn generated_client_has_correct_methods() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let config = test_config(tmp.path().to_path_buf());

    teleport_build::export_from_inventory(&config)
        .expect("generation should succeed");

    let client_ts = std::fs::read_to_string(tmp.path().join("client.ts"))
        .expect("client.ts should exist");

    // get_user is a query → GET
    assert!(
        client_ts.contains("/rpc/") && client_ts.contains("getUser"),
        "client.ts should contain getUser route"
    );

    // create_user is a command → POST
    assert!(
        client_ts.contains("POST"),
        "client.ts should contain POST method for command procedures"
    );

    // listUsers has no input → passes undefined
    assert!(
        client_ts.contains("undefined"),
        "client.ts should pass undefined for no-input procedures"
    );

    // Should import from @teleport-rs/client
    assert!(
        client_ts.contains("@teleport-rs/client"),
        "client.ts should import from @teleport-rs/client"
    );
}

#[test]
fn generated_errors_has_procedure_specific_aliases() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let config = test_config(tmp.path().to_path_buf());

    teleport_build::export_from_inventory(&config)
        .expect("generation should succeed");

    let errors_ts = std::fs::read_to_string(tmp.path().join("errors.ts"))
        .expect("errors.ts should exist");

    // get_user has AppError<GetUserError> — should produce an error alias.
    assert!(
        errors_ts.contains("GetUserError"),
        "errors.ts should contain GetUserError alias:\n{errors_ts}"
    );
}

#[test]
fn router_mounts_collected_procedures() {
    let router = TeleportRouter::new()
        .state(Arc::new(TestState))
        .mount();

    // The router should have been built successfully.
    // We can't easily inspect routes, but we verify it doesn't panic
    // and produces a valid Router.
    let _app: axum::Router = router;
}

#[test]
fn idempotent_generation() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let config = test_config(tmp.path().to_path_buf());

    teleport_build::export_from_inventory(&config)
        .expect("first generation should succeed");

    let types_first = std::fs::read_to_string(tmp.path().join("types.ts"))
        .expect("types.ts should exist");

    // Run generation again — files should not change.
    teleport_build::export_from_inventory(&config)
        .expect("second generation should succeed");

    let types_second = std::fs::read_to_string(tmp.path().join("types.ts"))
        .expect("types.ts should exist");

    assert_eq!(types_first, types_second, "generation should be idempotent");
}
