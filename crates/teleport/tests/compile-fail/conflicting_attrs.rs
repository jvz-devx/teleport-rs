use teleport::{remote, AppError};

#[derive(Clone)]
struct AppState;

#[remote(query, command)]
async fn bad(_ctx: &AppState) -> Result<String, AppError> {
    Ok("hello".into())
}

fn main() {}
