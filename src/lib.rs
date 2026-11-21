use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Result;
use axum::response::NoContent;
use axum::{Router as AxumRouter, Router, routing::get};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::compression::predicate::NotForContentType;
use tower_http::compression::{DefaultPredicate, Predicate};
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing::info;

use state::AppState;

use crate::controllers::rest::shows;

mod controllers;
mod datasource;
pub mod errors;
pub mod jobs;
pub mod models;
pub mod state;

static SOCKET: &SocketAddr = &SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8000);

fn setup_rustls() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");
}

pub async fn serve_axum(app_state: AppState) -> Result<()> {
    setup_rustls();
    let router = create_axum_router(app_state);
    let listener = TcpListener::bind(SOCKET).await?;
    info!("Listening on {SOCKET}");
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}

pub fn create_axum_router(app_state: AppState) -> AxumRouter {
    let compression_predicate =
        DefaultPredicate::new().and(NotForContentType::const_new("text/event-stream"));

    AxumRouter::new()
        // Unversioned on purpose: probes and load balancers should not depend on the API version.
        .route("/health", get(async || NoContent))
        .nest("/v1", v1_routes())
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new().compress_when(compression_predicate))
                .layer(DecompressionLayer::new()),
        )
}

pub fn v1_routes() -> Router<AppState> {
    use controllers::rest::{batch, downloads, episode, movie};

    AxumRouter::new()
        .nest(
            "/shows",
            AxumRouter::new()
                .route("/", get(shows::search))
                .route("/{id}", get(shows::by_id)),
        )
        .nest(
            "/downloads",
            AxumRouter::new()
                .route("/", get(downloads::find_downloads))
                .route("/updates", get(downloads::get_downloads_events))
                .nest(
                    "/batches",
                    AxumRouter::new()
                        .route("/", get(batch::find_downloads))
                        .route("/updates", get(batch::get_downloads_events)),
                )
                .nest(
                    "/episodes",
                    AxumRouter::new()
                        .route("/", get(episode::find_downloads))
                        .route("/updates", get(episode::get_downloads_events)),
                )
                .nest(
                    "/movies",
                    AxumRouter::new()
                        .route("/", get(movie::find_downloads))
                        .route("/updates", get(movie::get_downloads_events)),
                ),
        )
}
