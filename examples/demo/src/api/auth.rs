#![allow(clippy::unused_async)]

use teleport::{AppError, AuthedUser, remote};

use crate::state::AppState;
use crate::types::{LoginErrorDetail, LoginRequest, LoginResponse, User};

/// Authenticate with email and password.
#[remote(command)]
async fn login(
    ctx: &AppState,
    input: LoginRequest,
) -> Result<LoginResponse, AppError<LoginErrorDetail>> {
    let (token, user) = ctx
        .login(&input.email, &input.password)
        .ok_or(AppError::detail(LoginErrorDetail {
            invalid_credentials: true,
        }))?;

    Ok(LoginResponse {
        token: token.to_owned(),
        user: User {
            id: user.id.clone(),
            name: user.name.clone(),
            email: user.email.clone(),
            avatar: user.avatar.clone(),
        },
    })
}

/// Log out the current session (no-op in this demo).
#[remote(command)]
async fn logout(_ctx: &AppState, _auth: AuthedUser) -> Result<(), AppError> {
    // In a real app, invalidate the session token here.
    Ok(())
}

/// Get the currently authenticated user's profile.
#[remote(query)]
async fn get_profile(ctx: &AppState, auth: AuthedUser) -> Result<User, AppError> {
    ctx.get_user(&auth.id).cloned().ok_or(AppError::NotFound)
}
