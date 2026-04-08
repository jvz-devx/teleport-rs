#![allow(clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use teleport::{ExportConfig, TeleportRouter};
use tower_http::cors::CorsLayer;

mod api;
mod state;
mod types;

#[tokio::main]
async fn main() {
    TeleportRouter::<state::AppState>::export(
        &ExportConfig::new("frontend/src/lib/api/generated"),
    )
    .expect("failed to export TS bindings");

    let state = Arc::new(state::AppState::new());

    let app = TeleportRouter::new()
        .state(Arc::clone(&state))
        .auth("session", |token: String, state: Arc<state::AppState>| async move {
            state.validate_session(&token)
        })
        .manifest(true)
        .mount()
        .layer(CorsLayer::permissive()); // permissive for demo; use CorsLayer::new() with explicit origins in production

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");
    println!("Server running on http://localhost:3000");
    axum::serve(listener, app)
        .await
        .expect("server crashed");
}
