// `#[remote]` requires procedures to be `async`, but the test fixtures don't
// actually await anything — so silence `unused_async` at the file level.
// `expect_used` / `panic` are fine in tests — a panic is an assertion failure.
#![allow(clippy::unused_async, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};
use teleport::{
    AppError, AuthedUser, ProcedureRegistration, ProcedureType, TeleportRouter, TeleportUser,
    remote,
};

// A minimal state type for testing.
#[derive(Clone)]
struct AppState;

// Input/output types with required derives.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct UserId {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct User {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct CreateUserInput {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct GetUserError;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct CreateUserError;

// ---------------------------------------------------------------------------
// Procedure definitions
// ---------------------------------------------------------------------------

/// Fetch a single user by ID.
#[remote(query)]
async fn get_user(_ctx: &AppState, _input: UserId) -> Result<User, AppError<GetUserError>> {
    Ok(User {
        id: "1".into(),
        name: "Alice".into(),
    })
}

#[remote(query)]
async fn list_users(_ctx: &AppState) -> Result<Vec<User>, AppError<GetUserError>> {
    Ok(vec![])
}

#[remote(command)]
async fn create_user(
    _ctx: &AppState,
    _input: CreateUserInput,
) -> Result<User, AppError<CreateUserError>> {
    Ok(User {
        id: "2".into(),
        name: "Bob".into(),
    })
}

#[remote(query)]
async fn get_my_profile(
    _ctx: &AppState,
    _auth: AuthedUser,
) -> Result<User, AppError<GetUserError>> {
    Ok(User {
        id: "me".into(),
        name: "Me".into(),
    })
}

#[remote(query)]
async fn get_public_profile(
    _ctx: &AppState,
    _auth: Option<AuthedUser>,
    _input: UserId,
) -> Result<User, AppError<GetUserError>> {
    Ok(User {
        id: "1".into(),
        name: "Alice".into(),
    })
}

#[remote(query, name = "fetchUser")]
async fn get_user_renamed(_ctx: &AppState, _input: UserId) -> Result<User, AppError<GetUserError>> {
    Ok(User {
        id: "1".into(),
        name: "Alice".into(),
    })
}

#[remote(command, prefix = "admin")]
async fn delete_everything(_ctx: &AppState) -> Result<(), AppError> {
    Ok(())
}

// Custom user type for #[auth] attribute test.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CustomUser {
    user_id: i64,
}

impl TeleportUser for CustomUser {}

impl<S> FromRequestParts<S> for CustomUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(AppError::Unauthorized)
    }
}

#[remote(query)]
async fn get_custom_profile(
    _ctx: &AppState,
    #[auth] _user: CustomUser,
) -> Result<User, AppError<GetUserError>> {
    Ok(User {
        id: "custom".into(),
        name: "Custom".into(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn inventory_collects_procedures() {
    let procedures: Vec<&ProcedureRegistration> = inventory::iter::<ProcedureRegistration>
        .into_iter()
        .collect();

    // We defined 8 procedures above.
    assert_eq!(procedures.len(), 8, "expected 8 registered procedures");
}

#[test]
fn query_procedure_metadata() {
    let reg = find_by_fn_name("getUser");

    assert_eq!(reg.procedure_type, ProcedureType::Query);
    assert_eq!(reg.method, teleport::HttpMethod::Get);
    assert!(reg.doc.contains("Fetch a single user by ID"));
}

#[test]
fn command_procedure_metadata() {
    let reg = find_by_fn_name("createUser");

    assert_eq!(reg.procedure_type, ProcedureType::Command);
    assert_eq!(reg.method, teleport::HttpMethod::Post);
}

#[test]
fn name_override() {
    let reg = find_by_fn_name("fetchUser");

    assert_eq!(reg.fn_name, "fetchUser");
    assert!(reg.name().contains("fetchUser"));
}

#[test]
fn prefix_override() {
    let reg = find_by_fn_name("deleteEverything");

    assert_eq!(reg.prefix, Some("admin"));
    assert_eq!(reg.namespace(), "admin");
    assert!(reg.name().starts_with("admin."));
    assert!(reg.path().starts_with("/rpc/admin."));
}

#[test]
fn no_input_procedure() {
    let reg = find_by_fn_name("listUsers");
    assert_eq!(reg.procedure_type, ProcedureType::Query);
}

#[test]
fn router_builds_without_panic() {
    let _router = TeleportRouter::new().state(Arc::new(AppState)).mount();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[test]
fn auth_attribute_with_custom_user_type() {
    let reg = find_by_fn_name("getCustomProfile");

    // The procedure should be registered even though it uses a custom user type
    // via #[auth] instead of the built-in AuthedUser.
    assert_eq!(reg.procedure_type, ProcedureType::Query);
}

fn find_by_fn_name(name: &str) -> &'static ProcedureRegistration {
    inventory::iter::<ProcedureRegistration>
        .into_iter()
        .find(|r| r.fn_name == name)
        .unwrap_or_else(|| panic!("no procedure with fn_name = {name:?}"))
}

// ---------------------------------------------------------------------------
// Manifest tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn manifest_endpoint_returns_procedures() {
    use http::Request;
    use tower::ServiceExt;

    let router = TeleportRouter::new()
        .state(Arc::new(AppState))
        .manifest(true)
        .mount();

    let request = Request::builder()
        .uri("/rpc/__manifest")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("failed to build request");

    let response = router.oneshot(request).await.expect("request failed");

    assert_eq!(response.status(), http::StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .expect("missing content-type header");
    assert!(
        content_type
            .to_str()
            .unwrap_or("")
            .contains("application/json"),
        "expected application/json content-type"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("failed to read body");
    let manifest: serde_json::Value =
        serde_json::from_slice(&body).expect("invalid JSON in manifest");

    let procedures = manifest
        .get("procedures")
        .expect("missing 'procedures' key");
    assert!(procedures.is_object(), "procedures should be an object");

    // Verify a known query procedure is present with GET method.
    let get_user = procedures
        .get("remote.getUser")
        .expect("missing remote.getUser");
    assert_eq!(get_user["method"], "GET");
    assert_eq!(get_user["path"], "/rpc/remote.getUser");

    // Verify a known command procedure is present with POST method.
    let create_user = procedures
        .get("remote.createUser")
        .expect("missing remote.createUser");
    assert_eq!(create_user["method"], "POST");
    assert_eq!(create_user["path"], "/rpc/remote.createUser");

    // Verify prefix override is reflected.
    let admin = procedures
        .get("admin.deleteEverything")
        .expect("missing admin.deleteEverything");
    assert_eq!(admin["path"], "/rpc/admin.deleteEverything");
}

#[tokio::test]
async fn manifest_disabled_returns_404() {
    use http::Request;
    use tower::ServiceExt;

    let router = TeleportRouter::new()
        .state(Arc::new(AppState))
        .manifest(false)
        .mount();

    let request = Request::builder()
        .uri("/rpc/__manifest")
        .method("GET")
        .body(axum::body::Body::empty())
        .expect("failed to build request");

    let response = router.oneshot(request).await.expect("request failed");

    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
}
