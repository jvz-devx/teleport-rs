#![allow(clippy::unused_async)]

use teleport::{AppError, remote};

use crate::state::AppState;
use crate::types::{GetUserErrorDetail, User};

/// Fetch a single user by ID.
#[remote(query)]
async fn get_user(ctx: &AppState, id: String) -> Result<User, AppError<GetUserErrorDetail>> {
    ctx.get_user(&id)
        .cloned()
        .ok_or(AppError::detail(GetUserErrorDetail {
            user_not_found: true,
        }))
}

/// List all registered users.
#[remote(query)]
async fn list_users(ctx: &AppState) -> Result<Vec<User>, AppError> {
    Ok(ctx.list_users().to_vec())
}
