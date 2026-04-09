use teleport::{remote, AppError};

#[derive(Clone)]
struct AppState;

#[remote]
async fn missing(_ctx: &AppState) -> Result<String, AppError> {
    Ok("hello".into())
}

fn main() {}
