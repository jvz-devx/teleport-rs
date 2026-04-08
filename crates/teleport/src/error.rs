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
    Unauthorized,
    Forbidden,
    NotFound,
    BadRequest { message: String },
    Internal { message: String },
    RateLimited,
    Detail { detail: T },
}

impl<T> AppError<T> {
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

impl<T: Serialize> IntoResponse for AppError<T> {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = serde_json::to_string(&self).unwrap_or_default();
        (status, [("content-type", "application/json")], body).into_response()
    }
}
