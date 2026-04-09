use teleport::{remote, AppError};

#[derive(Clone)]
struct AppState;

#[derive(Clone)]
struct MyUser;

// Two `#[auth]` params are rejected by the macro with
// "duplicate auth parameter".
#[remote(query)]
async fn bad(
    _ctx: &AppState,
    #[auth] _a: MyUser,
    #[auth] _b: MyUser,
) -> Result<String, AppError> {
    Ok("hello".into())
}

fn main() {}
