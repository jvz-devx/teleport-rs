#![allow(clippy::expect_used, clippy::print_stdout)]

use std::{path::PathBuf, sync::Arc};

use teleport::{ExportConfig, TeleportRouter};
use tower_http::cors::CorsLayer;

mod api;
mod state;
mod types;

#[tokio::main]
async fn main() {
    let export_only = std::env::args().any(|arg| arg == "--export-only");
    let contract_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("teleport.contract.json");
    let export_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend/src/lib/api/generated");

    TeleportRouter::<state::AppState>::export(&ExportConfig::new(&export_dir))
        .expect("failed to export TS bindings");

    if export_only {
        TeleportRouter::<state::AppState>::export_contract(
            &contract_path,
            &ExportConfig::new(&export_dir),
        )
        .expect("failed to export contract");
        println!("Exported contract to {}", contract_path.display());
        println!("Exported TypeScript bindings to {}", export_dir.display());
        return;
    }

    let state = Arc::new(state::AppState::new());

    // Explicit CORS — see docs/security.md. Never use CorsLayer::permissive() in production.
    let cors = CorsLayer::new()
        .allow_origin(
            "http://localhost:5173"
                .parse::<http::HeaderValue>()
                .expect("valid origin"),
        )
        .allow_methods([http::Method::GET, http::Method::POST])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION])
        .allow_credentials(true);

    let app =
        TeleportRouter::new()
            .state(Arc::clone(&state))
            .auth(
                "session",
                |token: String, state: Arc<state::AppState>| async move {
                    state.validate_session(&token)
                },
            )
            .manifest(true)
            .mount()
            .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");
    println!("Server running on http://localhost:3000");
    axum::serve(listener, app).await.expect("server crashed");
}
