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
