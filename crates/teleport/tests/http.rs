#![allow(clippy::unwrap_used, clippy::expect_used, clippy::unused_async, clippy::panic)]

use std::sync::Arc;

use axum::body::Body;
use http::{Request, StatusCode};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;

use teleport::{remote, AppError, AuthedUser, TeleportRouter};

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct UserId {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
struct User {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct CreateUserInput {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct UserError;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct FilterInput {
    filter: Filter,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, PartialEq)]
struct Filter {
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct TagsInput {
    tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct OptionalSearch {
    q: Option<String>,
    page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct ValidationError {
    field: String,
    reason: String,
}

// ---------------------------------------------------------------------------
// Procedures
// ---------------------------------------------------------------------------

#[remote(query)]
async fn get_user(_ctx: &AppState, input: UserId) -> Result<User, AppError<UserError>> {
    Ok(User {
        id: input.id,
        name: "Test User".to_owned(),
    })
}

#[remote(command)]
async fn create_user(
    _ctx: &AppState,
    input: CreateUserInput,
) -> Result<User, AppError<UserError>> {
    Ok(User {
        id: "new-1".to_owned(),
        name: input.name,
    })
}

#[remote(query)]
async fn list_users(_ctx: &AppState) -> Result<Vec<User>, AppError<UserError>> {
    Ok(vec![
        User {
            id: "1".to_owned(),
            name: "Alice".to_owned(),
        },
        User {
            id: "2".to_owned(),
            name: "Bob".to_owned(),
        },
    ])
}

#[remote(query)]
async fn unauthorized_route(_ctx: &AppState) -> Result<(), AppError> {
    Err(AppError::Unauthorized)
}

#[remote(query)]
async fn not_found_route(_ctx: &AppState) -> Result<(), AppError> {
    Err(AppError::NotFound)
}

#[remote(query)]
async fn validation_error_route(
    _ctx: &AppState,
) -> Result<(), AppError<ValidationError>> {
    Err(AppError::Detail {
        detail: ValidationError {
            field: "email".to_owned(),
            reason: "invalid format".to_owned(),
        },
    })
}

#[remote(form)]
async fn submit_feedback(
    _ctx: &AppState,
    input: CreateUserInput,
) -> Result<User, AppError<UserError>> {
    Ok(User {
        id: "feedback-1".to_owned(),
        name: input.name,
    })
}

#[remote(query)]
async fn search_with_filter(
    _ctx: &AppState,
    input: FilterInput,
) -> Result<Filter, AppError> {
    Ok(input.filter)
}

#[remote(query)]
async fn search_with_tags(
    _ctx: &AppState,
    input: TagsInput,
) -> Result<Vec<String>, AppError> {
    Ok(input.tags)
}

#[remote(query)]
async fn search_optional(
    _ctx: &AppState,
    input: OptionalSearch,
) -> Result<OptionalSearch, AppError> {
    Ok(input)
}

#[remote(query)]
async fn get_my_profile(
    _ctx: &AppState,
    auth: AuthedUser,
) -> Result<User, AppError<UserError>> {
    Ok(User {
        id: auth.id,
        name: "Authenticated User".to_owned(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn app() -> axum::Router {
    TeleportRouter::new().state(Arc::new(AppState)).mount()
}

async fn response_json<T: serde::de::DeserializeOwned>(response: http::Response<Body>) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("failed to read response body");
    serde_json::from_slice(&body).expect("failed to deserialize response body")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_query_with_input() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getUser?id=123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "123");
    assert_eq!(user.name, "Test User");
}

#[tokio::test]
async fn post_command_with_json_body() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/rpc/http.createUser")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&CreateUserInput {
                        name: "Charlie".to_owned(),
                    })
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "new-1");
    assert_eq!(user.name, "Charlie");
}

#[tokio::test]
async fn no_input_query() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.listUsers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let users: Vec<User> = response_json(response).await;
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].name, "Alice");
}

#[tokio::test]
async fn app_error_unauthorized() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.unauthorizedRoute")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn app_error_not_found() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.notFoundRoute")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn app_error_with_detail() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.validationErrorRoute")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let error: AppError<ValidationError> = response_json(response).await;
    match error {
        AppError::Detail { detail } => {
            assert_eq!(detail.field, "email");
            assert_eq!(detail.reason, "invalid format");
        }
        other => panic!("expected Detail variant, got {other:?}"),
    }
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn auth_with_authed_user_succeeds() {
    let mut request = Request::builder()
        .uri("/rpc/http.getMyProfile")
        .body(Body::empty())
        .unwrap();

    request.extensions_mut().insert(AuthedUser {
        id: "user-42".to_owned(),
        email: "test@example.com".to_owned(),
    });

    let response = app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "user-42");
    assert_eq!(user.name, "Authenticated User");
}

#[tokio::test]
async fn auth_without_authed_user_returns_401() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getMyProfile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn form_procedure_accepts_post_json() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/rpc/http.submitFeedback")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&CreateUserInput {
                        name: "Feedback User".to_owned(),
                    })
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "feedback-1");
    assert_eq!(user.name, "Feedback User");
}

// ---------------------------------------------------------------------------
// QsQuery: bracket-notation query params
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qs_nested_object() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.searchWithFilter?filter[status]=active")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let filter: Filter = response_json(response).await;
    assert_eq!(filter, Filter { status: "active".to_owned() });
}

#[tokio::test]
async fn qs_array_params() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.searchWithTags?tags[0]=foo&tags[1]=bar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let tags: Vec<String> = response_json(response).await;
    assert_eq!(tags, vec!["foo", "bar"]);
}

#[tokio::test]
async fn qs_optional_fields_missing() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.searchOptional")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let result: OptionalSearch = response_json(response).await;
    assert_eq!(result.q, None);
    assert_eq!(result.page, None);
}

#[tokio::test]
async fn qs_optional_fields_present() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.searchOptional?q=hello&page=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let result: OptionalSearch = response_json(response).await;
    assert_eq!(result.q.as_deref(), Some("hello"));
    assert_eq!(result.page, Some(2));
}

#[tokio::test]
async fn qs_flat_params_still_work() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getUser?id=456")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "456");
}
