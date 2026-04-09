#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unused_async,
    clippy::panic
)]

use std::sync::Arc;

use axum::body::Body;
use axum::middleware as axum_mw;
use http::{Request, StatusCode};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;

use teleport::{AppError, AuthedUser, TeleportRouter, TeleportUser, remote};

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
async fn create_user(_ctx: &AppState, input: CreateUserInput) -> Result<User, AppError<UserError>> {
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
async fn validation_error_route(_ctx: &AppState) -> Result<(), AppError<ValidationError>> {
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
async fn search_with_filter(_ctx: &AppState, input: FilterInput) -> Result<Filter, AppError> {
    Ok(input.filter)
}

#[remote(query)]
async fn search_with_tags(_ctx: &AppState, input: TagsInput) -> Result<Vec<String>, AppError> {
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
async fn get_my_profile(_ctx: &AppState, auth: AuthedUser) -> Result<User, AppError<UserError>> {
    Ok(User {
        id: auth.id,
        name: "Authenticated User".to_owned(),
    })
}

#[remote(query)]
async fn get_optional_profile(
    _ctx: &AppState,
    auth: Option<AuthedUser>,
) -> Result<Option<String>, AppError> {
    Ok(auth.map(|u| u.email))
}

// ---------------------------------------------------------------------------
// Custom user type for generic auth tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CustomUser {
    user_id: i64,
    role: String,
}

impl TeleportUser for CustomUser {}

impl<S> axum::extract::FromRequestParts<S> for CustomUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
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
    #[auth] user: CustomUser,
) -> Result<User, AppError<UserError>> {
    Ok(User {
        id: user.user_id.to_string(),
        name: user.role,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn app() -> axum::Router {
    TeleportRouter::new().state(Arc::new(AppState)).mount()
}

/// Build a router with auth middleware that validates tokens against a static map.
fn app_with_auth() -> axum::Router {
    TeleportRouter::new()
        .state(Arc::new(AppState))
        .auth(
            "session",
            |token: String, _state: Arc<AppState>| async move {
                // Simple token → user mapping for tests.
                match token.as_str() {
                    "valid-token" => Some(AuthedUser {
                        id: "user-42".to_owned(),
                        email: "test@example.com".to_owned(),
                    }),
                    _ => None,
                }
            },
        )
        .mount()
}

/// Build a router with auth middleware returning a custom user type.
fn app_with_custom_auth() -> axum::Router {
    TeleportRouter::new()
        .state(Arc::new(AppState))
        .auth(
            "session",
            |token: String, _state: Arc<AppState>| async move {
                match token.as_str() {
                    "admin-token" => Some(CustomUser {
                        user_id: 99,
                        role: "admin".to_owned(),
                    }),
                    _ => None,
                }
            },
        )
        .mount()
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
    // Verifies the query-string `id` param round-trips into the handler's
    // `UserId` input and back through the `User` response — the handler
    // returns `user.id = input.id`, so this really is exercising deserialization.
    assert_eq!(
        user.id, "123",
        "query-string id should round-trip into the response"
    );
    // The name is a hardcoded handler constant; this assert only verifies
    // that the JSON body shape deserializes cleanly (smoke test).
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
    // Verify the body is a well-formed AppError::Unauthorized, not just any
    // 401 — the content-type must be JSON and the tagged union discriminant
    // must match. This catches regressions in the IntoResponse impl.
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json"),
    );
    let error: AppError = response_json(response).await;
    assert!(
        matches!(error, AppError::Unauthorized),
        "expected Unauthorized variant, got {error:?}",
    );
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
    // Verify the body is a well-formed AppError::NotFound, not just any 404.
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json"),
    );
    let error: AppError = response_json(response).await;
    assert!(
        matches!(error, AppError::NotFound),
        "expected NotFound variant, got {error:?}",
    );
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

#[tokio::test]
async fn form_procedure_accepts_urlencoded() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/rpc/http.submitFeedback")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=Form+User"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "feedback-1");
    assert_eq!(user.name, "Form User");
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
    assert_eq!(
        filter,
        Filter {
            status: "active".to_owned()
        }
    );
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

// ---------------------------------------------------------------------------
// Auth middleware integration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_middleware_cookie_valid_token() {
    let response = app_with_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getMyProfile")
                .header("cookie", "session=valid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "user-42");
    assert_eq!(user.name, "Authenticated User");
}

#[tokio::test]
async fn auth_middleware_bearer_valid_token() {
    let response = app_with_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getMyProfile")
                .header("authorization", "Bearer valid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "user-42");
}

#[tokio::test]
async fn auth_middleware_invalid_token_returns_401() {
    let response = app_with_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getMyProfile")
                .header("cookie", "session=bad-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_middleware_no_token_returns_401() {
    let response = app_with_auth()
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
async fn auth_middleware_optional_auth_with_token() {
    let response = app_with_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getOptionalProfile")
                .header("cookie", "session=valid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let email: Option<String> = response_json(response).await;
    assert_eq!(email.as_deref(), Some("test@example.com"));
}

#[tokio::test]
async fn auth_middleware_optional_auth_without_token() {
    let response = app_with_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getOptionalProfile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let email: Option<String> = response_json(response).await;
    assert_eq!(email, None);
}

// ---------------------------------------------------------------------------
// Per-route middleware via on_route
// ---------------------------------------------------------------------------

/// Middleware that adds an `X-Custom: applied` header to the response.
async fn tag_middleware(
    request: axum::extract::Request,
    next: axum_mw::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("x-custom", "applied".parse().unwrap());
    response
}

#[tokio::test]
async fn on_route_applies_middleware_to_matching_routes() {
    let app = TeleportRouter::new()
        .state(Arc::new(AppState))
        .on_route(|path, route| {
            if path.contains("getUser") {
                route.layer(axum_mw::from_fn(tag_middleware))
            } else {
                route
            }
        })
        .mount();

    // Matching route gets the custom header.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getUser?id=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-custom")
            .map(|v| v.to_str().unwrap()),
        Some("applied"),
    );

    // Non-matching route does NOT get the custom header.
    let response = app
        .oneshot(
            Request::builder()
                .uri("/rpc/http.listUsers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-custom").is_none());
}

// ---------------------------------------------------------------------------
// Custom user type auth middleware tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn custom_auth_valid_token() {
    let response = app_with_custom_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getCustomProfile")
                .header("cookie", "session=admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "99");
    assert_eq!(user.name, "admin");
}

#[tokio::test]
async fn custom_auth_invalid_token_returns_401() {
    let response = app_with_custom_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getCustomProfile")
                .header("cookie", "session=bad-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn custom_auth_no_token_returns_401() {
    let response = app_with_custom_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getCustomProfile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn custom_auth_bearer_token() {
    let response = app_with_custom_auth()
        .oneshot(
            Request::builder()
                .uri("/rpc/http.getCustomProfile")
                .header("authorization", "Bearer admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let user: User = response_json(response).await;
    assert_eq!(user.id, "99");
}

// ---------------------------------------------------------------------------
// Safety layers: body limit + panic recovery (Unit 1)
// ---------------------------------------------------------------------------

#[allow(clippy::panic)]
mod safety_layers {
    use std::sync::Arc;

    use axum::body::Body;
    use http::{Request, StatusCode};
    use serde::{Deserialize, Serialize};
    use tower::ServiceExt;

    use teleport::{AppError, TeleportRouter, remote};

    use super::AppState;

    #[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
    struct LargePayload {
        blob: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
    struct PayloadError;

    #[remote(command, prefix = "safety")]
    async fn echo_blob(
        _ctx: &AppState,
        input: LargePayload,
    ) -> Result<LargePayload, AppError<PayloadError>> {
        Ok(input)
    }

    #[remote(command, prefix = "safety")]
    async fn boom(
        _ctx: &AppState,
        _input: LargePayload,
    ) -> Result<LargePayload, AppError<PayloadError>> {
        panic!("boom");
    }

    /// Build a JSON body roughly `bytes` long. The exact byte count is
    /// `bytes + a few` because of the `{"blob":"..."}` framing — for body
    /// limit testing we just need it to be larger than the limit.
    fn payload_of(bytes: usize) -> Vec<u8> {
        let blob = "x".repeat(bytes);
        serde_json::to_vec(&LargePayload { blob }).unwrap()
    }

    fn default_app() -> axum::Router {
        TeleportRouter::new().state(Arc::new(AppState)).mount()
    }

    #[tokio::test]
    async fn test_default_body_limit_rejects_large_payload() {
        // 3 MiB > 2 MiB default limit.
        let body = payload_of(3 * 1024 * 1024);
        let response = default_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc/safety.echoBlob")
                    .header("content-type", "application/json")
                    .header("content-length", body.len().to_string())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn test_body_limit_override_accepts_larger_payload() {
        let app = TeleportRouter::new()
            .state(Arc::new(AppState))
            .body_limit(10 * 1024 * 1024)
            .mount();

        let body = payload_of(3 * 1024 * 1024);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc/safety.echoBlob")
                    .header("content-type", "application/json")
                    .header("content-length", body.len().to_string())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_no_body_limit_accepts_any_size() {
        let app = TeleportRouter::new()
            .state(Arc::new(AppState))
            .no_body_limit()
            .mount();

        // 5 MiB — well over the 2 MiB default; only succeeds if the limit
        // is fully removed.
        let body = payload_of(5 * 1024 * 1024);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc/safety.echoBlob")
                    .header("content-type", "application/json")
                    .header("content-length", body.len().to_string())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_panicking_handler_returns_500() {
        let body = serde_json::to_vec(&LargePayload {
            blob: "small".to_owned(),
        })
        .unwrap();
        let response = default_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rpc/safety.boom")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = std::str::from_utf8(&bytes).unwrap();
        // Panic payload must NOT leak into the response body.
        assert!(
            !body_str.contains("boom"),
            "response body leaked panic payload: {body_str}"
        );
        assert!(
            body_str.contains("internal server error"),
            "response body missing generic error message: {body_str}"
        );
    }

    // `test_no_catch_panic_escape_hatch`: a real end-to-end test of this
    // requires either spawning a subprocess (a panicking handler aborts the
    // current test process) or relying on `catch_unwind`, which doesn't
    // exercise the same code path tower-http uses internally. We instead
    // verify the builder method exists and constructs a router successfully;
    // the actual "panic propagates" behaviour is integration-tested manually.
    #[tokio::test]
    async fn test_no_catch_panic_escape_hatch_builds() {
        let _app = TeleportRouter::new()
            .state(Arc::new(AppState))
            .no_catch_panic()
            .mount();
        // integration-tested manually
    }
}

// ---------------------------------------------------------------------------
// Fallible auth middleware (try_auth) — Unit 2
// ---------------------------------------------------------------------------

#[allow(clippy::panic)]
mod try_auth_mod {
    use std::sync::Arc;

    use axum::body::Body;
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    use teleport::{AppError, TeleportRouter, remote};

    use super::AppState;

    // A dummy user type. We don't extract it in the procedure — we just care
    // about the middleware's short-circuit behaviour vs. pass-through.
    #[derive(Debug, Clone)]
    struct BannedUser;

    #[remote(query, prefix = "tryauth")]
    async fn get_secret(_ctx: &AppState) -> Result<String, AppError> {
        Ok("secret-value".to_owned())
    }

    fn app_with_try_auth() -> axum::Router {
        TeleportRouter::new()
            .state(Arc::new(AppState))
            .try_auth(
                "session",
                |_token: String, _state: Arc<AppState>| async move {
                    // Any token is rejected with Forbidden to exercise the
                    // short-circuit path.
                    Err::<BannedUser, _>(AppError::<()>::Forbidden)
                },
            )
            .mount()
    }

    #[tokio::test]
    async fn test_try_auth_custom_rejection() {
        // With a cookie present, the validator runs and returns Forbidden;
        // the middleware must short-circuit with a 403 response.
        let response = app_with_try_auth()
            .oneshot(
                Request::builder()
                    .uri("/rpc/tryauth.getSecret")
                    .header("cookie", "session=any-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        // With no cookie, the validator is never invoked and the request
        // passes through to the procedure, which returns the secret.
        let response = app_with_try_auth()
            .oneshot(
                Request::builder()
                    .uri("/rpc/tryauth.getSecret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let secret: String = serde_json::from_slice(&body).unwrap();
        assert_eq!(secret, "secret-value");
    }
}
