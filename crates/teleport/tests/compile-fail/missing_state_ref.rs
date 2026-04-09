use teleport::{remote, AppError};

#[derive(Clone)]
struct AppState;

#[remote(query)]
async fn no_ref(ctx: AppState) -> Result<String, AppError> {
    Ok("hello".into())
}

fn main() {}
