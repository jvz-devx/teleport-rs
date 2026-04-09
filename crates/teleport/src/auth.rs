use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

type InfallibleValidatorFn<S, U> =
    dyn Fn(String, Arc<S>) -> Pin<Box<dyn Future<Output = Option<U>> + Send>> + Send + Sync;

type FallibleValidatorFn<S, U> = dyn Fn(String, Arc<S>) -> Pin<Box<dyn Future<Output = Result<U, Response>> + Send>>
    + Send
    + Sync;

/// Internal shape of the validator configured on [`AuthConfig`].
///
/// Two variants are supported so callers can either silently pass through on
/// invalid tokens (letting extractors surface 401s) or short-circuit the
/// request with a custom rejection response.
pub(crate) enum ValidatorKind<S, U> {
    /// Returns `Option<U>`: on `None`, the middleware passes through and
    /// downstream extractors handle the missing user.
    Infallible(Arc<InfallibleValidatorFn<S, U>>),
    /// Returns `Result<U, Response>`: on `Err`, the middleware short-circuits
    /// with the provided response instead of running the remainder of the
    /// tower stack.
    Fallible(Arc<FallibleValidatorFn<S, U>>),
}

/// Configuration for the auth middleware.
///
/// Extracts a session token from cookies or the `Authorization: Bearer` header,
/// then calls a user-provided validator to resolve it into a user value of type `U`.
/// If validation succeeds, the user is inserted into request extensions.
///
/// Two validator shapes are supported:
///
/// - [`AuthConfig::new`] — infallible: returns `Option<U>`. Missing/invalid
///   tokens always pass through; procedure-level extractors surface 401s.
/// - [`AuthConfig::new_fallible`] — fallible: returns `Result<U, Response>`.
///   An `Err` short-circuits the request with the supplied response.
pub struct AuthConfig<S, U> {
    pub(crate) cookie_name: String,
    pub(crate) validator: ValidatorKind<S, U>,
}

impl<S, U> AuthConfig<S, U>
where
    S: Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
{
    /// Create a new auth configuration with an infallible validator.
    ///
    /// - `cookie_name`: the name of the session cookie to check first.
    /// - `validator`: an async function that receives a token string and shared
    ///   state, returning `Some(U)` if the token is valid.
    ///
    /// Missing or invalid tokens never block the request — downstream
    /// extractors (e.g. [`crate::AuthedUser`]) are responsible for returning
    /// `401 Unauthorized` when a procedure requires auth.
    pub fn new<F, Fut>(cookie_name: &str, validator: F) -> Self
    where
        F: Fn(String, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<U>> + Send + 'static,
    {
        Self {
            cookie_name: cookie_name.to_owned(),
            validator: ValidatorKind::Infallible(Arc::new(move |token, state| {
                Box::pin(validator(token, state))
            })),
        }
    }

    /// Create a new auth configuration with a fallible validator.
    ///
    /// Unlike [`Self::new`], the validator returns `Result<U, Response>`. When
    /// it returns `Err(response)`, the middleware short-circuits the request
    /// with that response instead of letting it reach the procedure handler.
    ///
    /// This is the building block behind [`crate::TeleportRouter::try_auth`],
    /// which wraps an `AppError<E>`-returning validator so user code doesn't
    /// have to construct `Response` values directly.
    pub fn new_fallible<F, Fut>(cookie_name: &str, validator: F) -> Self
    where
        F: Fn(String, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<U, Response>> + Send + 'static,
    {
        Self {
            cookie_name: cookie_name.to_owned(),
            validator: ValidatorKind::Fallible(Arc::new(move |token, state| {
                Box::pin(validator(token, state))
            })),
        }
    }
}

/// Axum middleware that extracts a token and validates it into a user of type `U`.
///
/// Behaviour depends on which [`ValidatorKind`] was configured:
///
/// - **Infallible**: if a token is present and validates to `Some(user)`, the
///   user is inserted into request extensions; otherwise the request passes
///   through unchanged.
/// - **Fallible**: if a token is present and validates to `Ok(user)`, the user
///   is inserted into request extensions; if the validator returns
///   `Err(response)`, the request is short-circuited with that response. If
///   no token is present, the request still passes through — letting
///   extractors (or absence of an auth extractor) decide what happens.
pub(crate) async fn auth_middleware<S, U>(
    State(config): State<Arc<AuthMiddlewareState<S, U>>>,
    mut request: Request,
    next: Next,
) -> Response
where
    S: Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
{
    if let Some(token) = extract_token(&request, &config.auth.cookie_name) {
        match &config.auth.validator {
            ValidatorKind::Infallible(validate) => {
                if let Some(user) = validate(token, Arc::clone(&config.app_state)).await {
                    request.extensions_mut().insert(user);
                }
            }
            ValidatorKind::Fallible(validate) => {
                match validate(token, Arc::clone(&config.app_state)).await {
                    Ok(user) => {
                        request.extensions_mut().insert(user);
                    }
                    Err(response) => return response,
                }
            }
        }
    }
    next.run(request).await
}

/// Combined state for the auth middleware layer, holding both the auth config
/// and the application state needed by the validator.
pub(crate) struct AuthMiddlewareState<S, U> {
    pub(crate) auth: AuthConfig<S, U>,
    pub(crate) app_state: Arc<S>,
}

/// Extract a session token from the request.
///
/// Checks the cookie header first (looking for `cookie_name=<value>`),
/// then falls back to `Authorization: Bearer <token>`.
fn extract_token(request: &Request, cookie_name: &str) -> Option<String> {
    // Check cookies first.
    if let Some(token) = extract_from_cookie(request, cookie_name) {
        return Some(token);
    }

    // Fall back to Authorization: Bearer header.
    extract_from_bearer(request)
}

fn extract_from_cookie(request: &Request, cookie_name: &str) -> Option<String> {
    let cookie_header = request.headers().get(http::header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{cookie_name}=");
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix(&prefix)
            && !value.is_empty()
        {
            return Some(value.to_owned());
        }
    }
    None
}

fn extract_from_bearer(request: &Request) -> Option<String> {
    let auth_header = request
        .headers()
        .get(http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let token = auth_header.strip_prefix("Bearer ")?;
    if token.is_empty() {
        return None;
    }
    Some(token.to_owned())
}

#[cfg(test)]
mod tests {
    // Test-only: `.expect()` in helpers is informative; panics are caught by the test runner.
    #![allow(clippy::expect_used)]

    use super::*;
    use axum::body::Body;

    fn make_request(headers: &[(&str, &str)]) -> Request {
        let mut builder = http::Request::builder().uri("/test");
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.body(Body::empty()).expect("building test request")
    }

    #[test]
    fn extracts_token_from_cookie() {
        let req = make_request(&[("cookie", "session=abc123")]);
        assert_eq!(extract_token(&req, "session"), Some("abc123".to_owned()));
    }

    #[test]
    fn extracts_token_from_cookie_with_multiple_cookies() {
        let req = make_request(&[("cookie", "other=xyz; session=abc123; another=456")]);
        assert_eq!(extract_token(&req, "session"), Some("abc123".to_owned()));
    }

    #[test]
    fn extracts_token_from_bearer_header() {
        let req = make_request(&[("authorization", "Bearer my-token")]);
        assert_eq!(extract_token(&req, "session"), Some("my-token".to_owned()));
    }

    #[test]
    fn cookie_takes_precedence_over_bearer() {
        let req = make_request(&[
            ("cookie", "session=cookie-token"),
            ("authorization", "Bearer bearer-token"),
        ]);
        assert_eq!(
            extract_token(&req, "session"),
            Some("cookie-token".to_owned())
        );
    }

    #[test]
    fn returns_none_when_no_token() {
        let req = make_request(&[]);
        assert_eq!(extract_token(&req, "session"), None);
    }

    #[test]
    fn returns_none_for_empty_cookie_value() {
        let req = make_request(&[("cookie", "session=")]);
        assert_eq!(extract_token(&req, "session"), None);
    }

    #[test]
    fn returns_none_for_empty_bearer_value() {
        let req = make_request(&[("authorization", "Bearer ")]);
        assert_eq!(extract_token(&req, "session"), None);
    }

    #[test]
    fn ignores_non_matching_cookie_name() {
        let req = make_request(&[("cookie", "other=abc123")]);
        assert_eq!(extract_token(&req, "session"), None);
    }

    #[test]
    fn custom_cookie_name() {
        let req = make_request(&[("cookie", "my_token=secret")]);
        assert_eq!(extract_token(&req, "my_token"), Some("secret".to_owned()));
    }
}
