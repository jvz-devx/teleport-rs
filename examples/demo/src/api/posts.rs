#![allow(clippy::unused_async)]

use teleport::{remote, AppError, AuthedUser};

use crate::state::AppState;
use crate::types::{CreatePostRequest, Post};

/// Get posts, optionally filtered by author ID.
#[remote(query)]
async fn get_posts(ctx: &AppState, author_id: String) -> Result<Vec<Post>, AppError> {
    Ok(ctx.get_posts(Some(&author_id)))
}

/// Create a new post (requires authentication).
#[remote(command)]
async fn create_post(
    ctx: &AppState,
    auth: AuthedUser,
    input: CreatePostRequest,
) -> Result<Post, AppError> {
    Ok(ctx.create_post(&auth.id, input.title, input.content, input.tags))
}
