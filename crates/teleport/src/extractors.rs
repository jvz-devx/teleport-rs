use axum::extract::FromRequestParts;
use axum::extract::OptionalFromRequestParts;
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Represents an authenticated user, extracted from request extensions
/// by the auth middleware.
///
/// Use as a function parameter in `#[remote]` procedures to require
/// authentication. Use `Option<AuthedUser>` for optional auth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthedUser {
    pub id: String,
    pub email: String,
}

impl<S> FromRequestParts<S> for AuthedUser
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

impl<S> OptionalFromRequestParts<S> for AuthedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(parts.extensions.get::<Self>().cloned())
    }
}
