//! Extractor error-path integration tests.
//!
//! Exercises the rejection paths of `FormOrJson`, `QsQuery`, and axum's
//! `Json<T>` extractor used by `#[remote]` procedures. The happy paths are
//! covered in `http.rs`; these tests verify that malformed input produces a
//! structured `AppError` body with a 4xx status, not a panic or 500.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unused_async,
    clippy::panic
)]

use std::sync::Arc;

use axum::body::Body;
use http::{Request, StatusCode};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;

use teleport::{AppError, TeleportRouter, remote};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct Echo {
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
struct RequiredQuery {
    id: String,
}

#[remote(command, prefix = "extract_err")]
async fn echo_command(_ctx: &AppState, input: Echo) -> Result<Echo, AppError> {
    Ok(input)
}

#[remote(form, prefix = "extract_err")]
async fn echo_form(_ctx: &AppState, input: Echo) -> Result<Echo, AppError> {
    Ok(input)
}

#[remote(query, prefix = "extract_err")]
async fn echo_query(_ctx: &AppState, input: RequiredQuery) -> Result<RequiredQuery, AppError> {
    Ok(input)
}

fn app() -> axum::Router {
    TeleportRouter::new().state(Arc::new(AppState)).mount()
}

async fn response_json<T: serde::de::DeserializeOwned>(response: http::Response<Body>) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("failed to read response body");
    serde_json::from_slice(&body)
        .unwrap_or_else(|e| panic!("failed to deserialize response body: {e}"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Assert the response is a client error (4xx), not a success or server
/// error. axum's body extractors map different failures to different 4xx
/// codes (400, 415, 422), and the exact code is a property of the
/// dependency — we verify it stays in the 4xx family first, then tighten to
/// the expected category below.
fn assert_is_client_error(status: StatusCode) {
    assert!(
        status.is_client_error(),
        "expected 4xx client error, got {status}",
    );
}

// ---------------------------------------------------------------------------
// Malformed JSON body → 400 BadRequest
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_malformed_json_body_returns_400() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/rpc/extract_err.echoCommand")
                .header("content-type", "application/json")
                .body(Body::from("{not valid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    // `#[remote(command)]` uses axum's built-in `Json<T>` extractor, whose
    // rejection is handled by axum itself (our handler is never called), so
    // the status and body come from `JsonRejection::into_response`. Axum
    // currently returns 400 for a syntactically invalid JSON body and 422
    // for a syntactically valid body whose shape doesn't match — either is
    // a client error, but the exact code is axum's choice. We assert on the
    // category first, then pin to the current code below.
    assert_is_client_error(response.status());
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "axum returns 400 for syntactically-invalid JSON; if it ever switches \
         to 422, update this assertion",
    );

    // The body is axum's plaintext rejection — verify it at least mentions
    // the failure reason so a client sees a useful error message, rather
    // than an empty body or a leaked internal panic.
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = std::str::from_utf8(&body).unwrap().to_lowercase();
    assert!(
        body_str.contains("json") || body_str.contains("syntax") || body_str.contains("expected"),
        "axum JSON rejection body should describe the parse failure, got: {body_str}",
    );
}

// ---------------------------------------------------------------------------
// Invalid form body → 400 BadRequest
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_invalid_form_body_returns_400() {
    // The form deserializer needs a `value` field of type String; sending
    // a completely different key should cause deserialization to fail.
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/rpc/extract_err.echoForm")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("not_the_right_key=anything"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_is_client_error(response.status());
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let error: AppError = response_json(response).await;
    assert!(
        matches!(error, AppError::BadRequest { .. }),
        "expected BadRequest variant, got {error:?}",
    );
}

// ---------------------------------------------------------------------------
// Garbage query string → 400 BadRequest
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_garbage_query_string_returns_400() {
    // `?id[[` is malformed bracket-notation that `serde_qs` rejects during
    // parsing (unclosed bracket group).
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/extract_err.echoQuery?id[[invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_is_client_error(response.status());
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let error: AppError = response_json(response).await;
    assert!(
        matches!(error, AppError::BadRequest { .. }),
        "expected BadRequest variant, got {error:?}",
    );
}

// ---------------------------------------------------------------------------
// Missing required query field → 400 BadRequest
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_missing_required_query_field_returns_400() {
    // No query string at all; `RequiredQuery` needs `id`, so serde_qs should
    // fail with "missing field `id`".
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/rpc/extract_err.echoQuery")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_is_client_error(response.status());
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let error: AppError = response_json(response).await;
    match error {
        AppError::BadRequest { message } => {
            // The error message should mention the missing field — if it
            // ever stops doing that, the user-facing error quality has
            // regressed. This is a soft check via substring so minor
            // serde_qs message tweaks don't break the test.
            assert!(
                message.contains("id") || message.to_lowercase().contains("missing"),
                "expected missing-field message mentioning `id`, got: {message}",
            );
        }
        other => panic!("expected BadRequest variant, got {other:?}"),
    }
}
