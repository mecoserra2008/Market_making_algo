pub mod handlers;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod state;
pub mod websocket;

pub use state::AppState;

use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Start the API server
pub async fn start_server(state: AppState, port: u16) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("Starting API server on {}", addr);

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("API server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Create the Axum router with all routes and middleware
fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(routes::api_routes())
        .merge(routes::websocket_routes())
        .layer(CorsLayer::permissive()) // TODO: Configure proper CORS in production
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
