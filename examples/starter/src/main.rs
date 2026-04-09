// Minimal teleport-rs example. See examples/demo for a fuller walkthrough.
#![allow(clippy::expect_used, clippy::unused_async, clippy::print_stdout)]

use std::sync::Arc;

use teleport::{AppError, ExportConfig, TeleportRouter, remote, teleport_type};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState;

#[teleport_type]
struct Greeting {
    message: String,
}

#[remote(query)]
async fn hello(_ctx: &AppState, name: String) -> Result<Greeting, AppError> {
    Ok(Greeting {
        message: format!("hello, {name}"),
    })
}

#[remote(command)]
async fn shout(_ctx: &AppState, input: Greeting) -> Result<Greeting, AppError> {
    Ok(Greeting {
        message: input.message.to_uppercase(),
    })
}

#[tokio::main]
async fn main() {
    TeleportRouter::<AppState>::export(&ExportConfig::new("bindings"))
        .expect("failed to export TS bindings");

    // Explicit CORS — never use CorsLayer::permissive() in production.
    let cors = CorsLayer::new()
        .allow_origin(
            "http://localhost:5173"
                .parse::<http::HeaderValue>()
                .expect("valid origin"),
        )
        .allow_methods([http::Method::GET, http::Method::POST])
        .allow_headers([http::header::CONTENT_TYPE]);

    let app = TeleportRouter::new()
        .state(Arc::new(AppState))
        .mount()
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("bind");
    println!("listening on http://localhost:3000");
    axum::serve(listener, app).await.expect("server crashed");
}
