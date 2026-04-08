use std::sync::Arc;

use serde::{Deserialize, Serialize};
use teleport::{remote, AppError, AuthedUser, ProcedureRegistration, ProcedureType, TeleportRouter};

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
async fn get_user_renamed(
    _ctx: &AppState,
    _input: UserId,
) -> Result<User, AppError<GetUserError>> {
    Ok(User {
        id: "1".into(),
        name: "Alice".into(),
    })
}

#[remote(command, prefix = "admin")]
async fn delete_everything(_ctx: &AppState) -> Result<(), AppError> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn inventory_collects_procedures() {
    let procedures: Vec<&ProcedureRegistration> =
        inventory::iter::<ProcedureRegistration>.into_iter().collect();

    // We defined 7 procedures above.
    assert_eq!(procedures.len(), 7, "expected 7 registered procedures");
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
    let _router = TeleportRouter::new()
        .state(Arc::new(AppState))
        .mount();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_by_fn_name(name: &str) -> &'static ProcedureRegistration {
    inventory::iter::<ProcedureRegistration>
        .into_iter()
        .find(|r| r.fn_name == name)
        .unwrap_or_else(|| panic!("no procedure with fn_name = {name:?}"))
}
