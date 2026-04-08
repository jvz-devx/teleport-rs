use axum::extract::{FromRequest, FromRequestParts, OptionalFromRequestParts, Request};
use axum::http::request::Parts;
use axum::{Form, Json};
use serde::de::DeserializeOwned;
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

/// Marker trait for types that can be used as authenticated user in `#[remote]` procedures.
///
/// Implement this for your custom user type alongside [`FromRequestParts`].
/// The built-in [`AuthedUser`] already implements this trait.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Clone)]
/// struct MyUser { id: i64 }
///
/// impl TeleportUser for MyUser {}
/// ```
pub trait TeleportUser: Clone + Send + Sync + 'static {}

impl TeleportUser for AuthedUser {}

/// Extractor that accepts both JSON and URL-encoded form data.
///
/// Checks `Content-Type` to decide how to deserialize the request body:
/// - `application/x-www-form-urlencoded` → form deserialization
/// - anything else (including `application/json`) → JSON deserialization
///
/// This enables progressive enhancement: HTML forms submit url-encoded data
/// natively, while JS clients can send JSON.
pub struct FormOrJson<T>(pub T);

impl<S, T> FromRequest<S> for FormOrJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + 'static,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_form = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("application/x-www-form-urlencoded"));

        if is_form {
            let Form(data) = Form::<T>::from_request(req, state)
                .await
                .map_err(|e| AppError::BadRequest {
                    message: e.to_string(),
                })?;
            Ok(Self(data))
        } else {
            let Json(data) = Json::<T>::from_request(req, state)
                .await
                .map_err(|e| AppError::BadRequest {
                    message: e.to_string(),
                })?;
            Ok(Self(data))
        }
    }
}

/// Query parameter extractor using `serde_qs` for bracket-notation support.
///
/// Unlike Axum's built-in `Query<T>` (which uses `serde_urlencoded`), this
/// handles nested objects (`filter[status]=active`) and arrays
/// (`tags[0]=foo&tags[1]=bar`) as produced by JavaScript's `qs.stringify()`.
pub struct QsQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for QsQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        let value = serde_qs::from_str(query).map_err(|e| AppError::BadRequest {
            message: e.to_string(),
        })?;
        Ok(Self(value))
    }
}
