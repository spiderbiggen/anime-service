use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use axum::{routing::get, Router as AxumRouter, Router};
use reqwest::header::CONTENT_TYPE;
use tokio::net::TcpListener;
use tokio::sync::broadcast::Sender;
use tower::make::Shared;
use tower::steer::Steer;
use tower::ServiceBuilder;
use tower_http::compression::predicate::NotForContentType;
use tower_http::compression::{DefaultPredicate, Predicate};
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing::info;

use state::AppState;

use crate::controllers::rest::anime;

mod controllers;
mod datasource;
pub mod errors;
pub mod jobs;
pub mod models;
pub mod state;

static SOCKET: &SocketAddr = &SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 8000);

pub async fn serve_axum(app_state: AppState) -> Result<()> {
    let router = create_axum_router(app_state);
    let listener = TcpListener::bind(SOCKET).await?;
    tracing::debug!("listening on {SOCKET}");
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}

pub async fn serve_tonic(sender: Sender<models::DownloadGroup>) -> Result<()> {
    let router = create_tonic_router(sender);
    let listener = TcpListener::bind(SOCKET).await?;
    info!("Listening on {SOCKET}");
    axum::serve(listener, router).await?;
    Ok(())
}

pub async fn serve_combined(app_state: AppState) -> Result<()> {
    let tonic_router = create_tonic_router(app_state.downloads_channel.clone());
    let axum_router = create_axum_router(app_state);

    let http_grpc = Steer::new(
        vec![axum_router, tonic_router],
        |req: &Request<Body>, _services: &[_]| {
            let is_grpc = req
                .headers()
                .get(CONTENT_TYPE)
                .map(|content_type| content_type.as_bytes())
                .filter(|content_type| content_type.starts_with(b"application/grpc"))
                .is_some();
            // 0 -> http, 1 -> grpc
            usize::from(is_grpc)
        },
    );

    let listener = TcpListener::bind(SOCKET).await?;
    info!("Listening on {SOCKET}");
    axum::serve(listener, Shared::new(http_grpc)).await?;
    Ok(())
}

pub fn create_axum_router(app_state: AppState) -> AxumRouter {
    let compression_predicate =
        DefaultPredicate::new().and(NotForContentType::const_new("text/event-stream"));

    AxumRouter::new()
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
                .route("/", get(anime::find))
                .route("/{id}", get(anime::by_id)),
        )
        .nest(
            "/downloads",
            AxumRouter::new()
                .route("/", get(downloads::find_downloads))
                .route("/updates", get(downloads::get_downloads_events))
                .nest(
                    "/anime",
                    AxumRouter::new()
                        .route("/", get(batch::find_downloads))
                        .route("/updates", get(batch::get_downloads_events)),
                )
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

pub fn create_tonic_router(sender: Sender<models::DownloadGroup>) -> Router {
    use controllers::grpc::DownloadService;
    use proto::api::v1::downloads_server::DownloadsServer as V1DownloadsServer;
    use proto::api::v2::downloads_server::DownloadsServer as V2DownloadsServer;

    let service = Arc::new(DownloadService { sender });
    let mut builder = tonic::service::Routes::builder();
    builder
        .add_service(V1DownloadsServer::from_arc(Arc::clone(&service)))
        .add_service(V2DownloadsServer::from_arc(service));
    builder.routes().into_axum_router()
}
