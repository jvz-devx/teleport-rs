use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

/// Framework error type. `T` is the procedure-specific error detail.
///
/// Shared variants cover common HTTP error cases. The `Detail` variant
/// carries procedure-specific information typed by `T`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type")]
pub enum AppError<T = ()> {
    /// No valid session was provided. Maps to `401 Unauthorized`.
    Unauthorized,
    /// Authenticated but not permitted. Maps to `403 Forbidden`.
    Forbidden,
    /// The requested resource does not exist. Maps to `404 Not Found`.
    NotFound,
    /// Input validation failed. Maps to `400 Bad Request`.
    BadRequest {
        /// Human-readable reason the request was rejected.
        message: String,
    },
    /// Unexpected server error. Maps to `500 Internal Server Error`.
    Internal {
        /// Internal message for logs; also returned in the JSON body.
        message: String,
    },
    /// Too many requests; the client should back off. Maps to `429 Too Many Requests`.
    RateLimited,
    /// Procedure-specific error typed by `T`. Maps to `422 Unprocessable Entity`.
    Detail {
        /// Procedure-specific error payload.
        detail: T,
    },
}

impl<T> From<T> for AppError<T> {
    fn from(detail: T) -> Self {
        Self::Detail { detail }
    }
}

impl<T> AppError<T> {
    /// Create a `Detail` variant from a procedure-specific error value.
    pub const fn detail(detail: T) -> Self {
        Self::Detail { detail }
    }

    const fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::Detail { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}

#[allow(clippy::print_stderr)]
impl<T: Serialize> IntoResponse for AppError<T> {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = match serde_json::to_string(&self) {
            Ok(json) => json,
            Err(err) => {
                eprintln!("teleport-rs: failed to serialize AppError: {err}");
                r#"{"type":"Internal","message":"error serialization failed"}"#.to_owned()
            }
        };
        (status, [("content-type", "application/json")], body).into_response()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use axum::body;
    use serde::Serialize;
    use serde_json::{Value, json};

    use super::*;

    #[derive(Debug, Clone, Serialize)]
    struct DetailPayload {
        code: u16,
    }

    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("boom"))
        }
    }

    fn response_json<T: Serialize>(error: AppError<T>) -> (StatusCode, Value) {
        let response = error.into_response();
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .expect("content-type");
        assert_eq!(content_type, "application/json");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let body = runtime
            .block_on(body::to_bytes(response.into_body(), usize::MAX))
            .expect("read response body");
        let json = serde_json::from_slice(&body).expect("parse response json");
        (status, json)
    }

    #[test]
    fn shared_variants_map_to_expected_status_codes_and_payloads() {
        let cases = [
            (
                AppError::<()>::Unauthorized,
                StatusCode::UNAUTHORIZED,
                json!({ "type": "Unauthorized" }),
            ),
            (
                AppError::<()>::Forbidden,
                StatusCode::FORBIDDEN,
                json!({ "type": "Forbidden" }),
            ),
            (
                AppError::<()>::NotFound,
                StatusCode::NOT_FOUND,
                json!({ "type": "NotFound" }),
            ),
            (
                AppError::<()>::BadRequest {
                    message: "bad input".to_owned(),
                },
                StatusCode::BAD_REQUEST,
                json!({ "type": "BadRequest", "message": "bad input" }),
            ),
            (
                AppError::<()>::Internal {
                    message: "db offline".to_owned(),
                },
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "type": "Internal", "message": "db offline" }),
            ),
            (
                AppError::<()>::RateLimited,
                StatusCode::TOO_MANY_REQUESTS,
                json!({ "type": "RateLimited" }),
            ),
        ];

        for (error, expected_status, expected_json) in cases {
            let (status, json) = response_json(error);
            assert_eq!(status, expected_status);
            assert_eq!(json, expected_json);
        }
    }

    #[test]
    fn detail_variant_serializes_payload_from_helpers() {
        let expected = json!({
            "type": "Detail",
            "detail": { "code": 422 }
        });

        let from_impl: AppError<DetailPayload> = DetailPayload { code: 422 }.into();
        let (status, json) = response_json(from_impl);
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json, expected);

        let from_ctor = AppError::detail(DetailPayload { code: 422 });
        let (status, json) = response_json(from_ctor);
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json, expected);
    }

    #[test]
    fn falls_back_to_internal_body_when_error_payload_cannot_serialize() {
        let response = AppError::detail(FailingSerialize).into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let body = runtime
            .block_on(body::to_bytes(response.into_body(), usize::MAX))
            .expect("read response body");

        assert_eq!(
            std::str::from_utf8(&body).expect("utf8 body"),
            r#"{"type":"Internal","message":"error serialization failed"}"#
        );
    }
}
