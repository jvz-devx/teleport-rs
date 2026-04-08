use teleport::{remote, AppError};

struct Handler;

impl Handler {
    #[remote(query)]
    async fn with_self(&self) -> Result<String, AppError> {
        Ok("hello".into())
    }
}

fn main() {}
