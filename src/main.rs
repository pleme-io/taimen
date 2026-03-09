//! Taimen signaling server entry point.
//!
//! ```text
//! cargo run                             # start server on 0.0.0.0:8443
//! cargo run -- server                   # explicit server subcommand
//! cargo run -- mcp                      # MCP admin server (stdio)
//! TAIMEN_PORT=3000 cargo run            # custom port
//! RUST_LOG=debug cargo run              # with debug tracing
//! ```

use std::net::SocketAddr;

use axum::routing::get;
use axum::Router;
use clap::{Parser, Subcommand};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use taimen::api;
use taimen::app_state::AppState;
use taimen::signaling;

#[derive(Parser)]
#[command(name = "taimen", version, about = "Open-source video conferencing server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the signaling server (default)
    Server,
    /// Start MCP admin server (stdio transport)
    Mcp,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::Server) => {
            run_server().await?;
        }
        Some(Commands::Mcp) => {
            taimen::mcp::run()
                .await
                .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))?;
        }
    }

    Ok(())
}

async fn run_server() -> anyhow::Result<()> {
    // Initialise tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let jwt_secret =
        std::env::var("TAIMEN_JWT_SECRET").unwrap_or_else(|_| "taimen-dev-secret-change-me".into());
    let port: u16 = std::env::var("TAIMEN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8443);

    let state = AppState::new(jwt_secret);

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws/{room_id}", get(signaling::ws_handler))
        .merge(api::router())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("taimen signaling server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}
