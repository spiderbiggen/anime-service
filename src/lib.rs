use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6};

use anyhow::Result;
use axum::{routing::get, Router};
use proto::api::downloads_server::DownloadsServer;
use tokio::sync::broadcast::Sender;
use tower::ServiceBuilder;
use tower_http::compression::predicate::NotForContentType;
use tower_http::compression::{DefaultPredicate, Predicate};
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};

use state::AppState;

use crate::controllers::{anime, downloads};

pub mod controllers;
pub mod datasource;
pub mod errors;
pub mod jobs;
pub mod models;
pub mod request_cache;
pub mod state;

const ADDRESS: &SocketAddr = &SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 8000);

pub async fn serve_axum(app_state: AppState) -> Result<()> {
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

    tracing::debug!("listening on {}", ADDRESS);
    axum::Server::bind(&ADDRESS)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

pub async fn serve_tonic(sender: Sender<models::DownloadGroup>) -> Result<()> {
    let svc = DownloadsServer::new(controllers::downloads::DownloadService { sender });

    tonic::transport::Server::builder()
        .add_service(svc)
        .serve(ADDRESS.clone())
        .await?;
    Ok(())
}
