use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::extractors::AuthedUser;

type ValidatorFn<S> = dyn Fn(String, Arc<S>) -> Pin<Box<dyn Future<Output = Option<AuthedUser>> + Send>>
    + Send
    + Sync;

/// Configuration for the auth middleware.
///
/// Extracts a session token from cookies or the `Authorization: Bearer` header,
/// then calls a user-provided validator to resolve it into an [`AuthedUser`].
/// If validation succeeds, the user is inserted into request extensions.
/// The request always proceeds — procedure-level extractors handle 401 responses.
pub struct AuthConfig<S> {
    pub(crate) cookie_name: String,
    pub(crate) validator: Arc<ValidatorFn<S>>,
}

impl<S> AuthConfig<S>
where
    S: Send + Sync + 'static,
{
    /// Create a new auth configuration.
    ///
    /// - `cookie_name`: the name of the session cookie to check first.
    /// - `validator`: an async function that receives a token string and shared
    ///   state, returning `Some(AuthedUser)` if the token is valid.
    pub fn new<F, Fut>(cookie_name: &str, validator: F) -> Self
    where
        F: Fn(String, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<AuthedUser>> + Send + 'static,
    {
        Self {
            cookie_name: cookie_name.to_owned(),
            validator: Arc::new(move |token, state| Box::pin(validator(token, state))),
        }
    }
}

/// Axum middleware that extracts a token and validates it into an [`AuthedUser`].
pub(crate) async fn auth_middleware<S: Send + Sync + 'static>(
    State(config): State<Arc<AuthMiddlewareState<S>>>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(token) = extract_token(&request, &config.auth.cookie_name)
        && let Some(user) = (config.auth.validator)(token, Arc::clone(&config.app_state)).await
    {
        request.extensions_mut().insert(user);
    }
    next.run(request).await
}

/// Combined state for the auth middleware layer, holding both the auth config
/// and the application state needed by the validator.
pub(crate) struct AuthMiddlewareState<S> {
    pub(crate) auth: AuthConfig<S>,
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
        assert_eq!(
            extract_token(&req, "my_token"),
            Some("secret".to_owned())
        );
    }
}
