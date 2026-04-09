use teleport::remote;

#[derive(Clone)]
struct AppState;

#[remote(query)]
async fn bad_ret(_ctx: &AppState) -> String {
    "hello".into()
}

fn main() {}
