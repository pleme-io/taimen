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
    // Initialise tracing via shidou — honors RUST_LOG, defaults to info.
    shidou::init_tracing();

    let jwt_secret =
        std::env::var("TAIMEN_JWT_SECRET").unwrap_or_else(|_| "taimen-dev-secret-change-me".into());
    let port: u16 = std::env::var("TAIMEN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8443);

    let state = AppState::new(jwt_secret);

    // Health/readiness/liveness via the shared tsunagu router (JSON
    // responses, Unhealthy → 503 for K8s probes).
    let health_checker: std::sync::Arc<dyn tsunagu::HealthChecker> =
        std::sync::Arc::new(tsunagu::SimpleHealthChecker::new("taimen", env!("CARGO_PKG_VERSION")));

    let app = Router::new()
        .merge(tsunagu::axum::health_router::<AppState>(health_checker))
        .route("/ws/{room_id}", get(signaling::ws_handler))
        .merge(api::router())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("taimen signaling server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Drain coordinator from tsunagu — installs SIGTERM/SIGINT handlers once
    // and hands out independent tokens. Upgrade over the previous ctrl_c-only
    // handler: SIGTERM now triggers graceful drain under Kubernetes / systemd.
    let drain = tsunagu::ShutdownController::install();
    axum::serve(listener, app)
        .with_graceful_shutdown(drain.token().wait())
        .await?;

    Ok(())
}

