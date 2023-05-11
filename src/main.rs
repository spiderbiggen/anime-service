use std::net::Ipv4Addr;

use anyhow::Result;
use axum::{routing::get, Router};
use tower::ServiceBuilder;
use tower_http::compression::predicate::NotForContentType;
use tower_http::compression::{DefaultPredicate, Predicate};
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use state::AppState;

use crate::controllers::{anime, downloads};

mod controllers;
mod datasource;
mod errors;
mod jobs;
mod models;
mod request_cache;
mod state;

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_state = AppState::new().await?;
    sqlx::migrate!().run(&app_state.pool).await?;
    jobs::poller::start(app_state.clone()).await?;

    let compression_predicate =
        DefaultPredicate::new().and(NotForContentType::const_new("text/event-stream"));
    // our router
    let app = Router::new()
        .route("/series", get(anime::get_collection))
        .route("/series/:id", get(anime::get_single))
        .route("/downloads", get(downloads::get))
        .route("/downloads/updates", get(downloads::get_updates))
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new().compress_when(compression_predicate))
                .layer(DecompressionLayer::new()),
        );

    let addr = (Ipv4Addr::UNSPECIFIED, 8000).into();
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}
