use std::net::SocketAddr;

use axum::{routing::get, Router};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use state::AppState;

use crate::controllers::{anime, downloads};
use crate::errors::InternalError;

mod controllers;
mod datasource;
mod errors;
mod jobs;
mod models;
mod request_cache;
mod state;

#[tokio::main]
async fn main() -> Result<(), InternalError> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_state = AppState::default();
    sqlx::migrate!().run(&app_state.pool).await?;
    jobs::poller::start(app_state.clone())?;

    // our router
    let app = Router::new()
        .route("/series", get(anime::get_collection))
        .route("/series/:id", get(anime::get_single))
        .route("/downloads", get(downloads::get))
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(DecompressionLayer::new()),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
