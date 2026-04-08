use teleport::{remote, AppError};

#[derive(Clone)]
struct AppState;

#[remote(query)]
fn not_async(_ctx: &AppState) -> Result<String, AppError> {
    Ok("hello".into())
}

fn main() {}
