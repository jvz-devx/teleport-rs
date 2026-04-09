#![allow(clippy::unused_async)]

use teleport::{AppError, AuthedUser, remote, teleport_type};

use crate::state::AppState;
use crate::types::{CreatePostRequest, Post};

/// Query input for `get_posts`.
///
/// Query inputs must be struct wrappers — `serde_qs` cannot deserialize
/// bare primitive types, so even a single-field input needs its own struct.
#[teleport_type]
pub struct GetPostsByAuthor {
    pub author_id: String,
}

/// Get posts, optionally filtered by author ID.
#[remote(query)]
async fn get_posts(ctx: &AppState, input: GetPostsByAuthor) -> Result<Vec<Post>, AppError> {
    Ok(ctx.get_posts(Some(&input.author_id)))
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
