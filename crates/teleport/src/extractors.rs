use serde::{Deserialize, Serialize};

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
