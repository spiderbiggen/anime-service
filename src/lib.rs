use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use axum::{routing::get, Router as AxumRouter};
use reqwest::header::CONTENT_TYPE;
use tokio::sync::broadcast::Sender;
use tonic::transport::server::Router as TonicRouter;
use tower::make::Shared;
use tower::steer::Steer;
use tower::{ServiceBuilder, ServiceExt};
use tower_http::compression::predicate::NotForContentType;
use tower_http::compression::{DefaultPredicate, Predicate};
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing::info;

use state::AppState;

mod controllers;
mod datasource;
pub mod errors;
pub mod jobs;
pub mod models;
pub mod state;

const ADDRESS: &SocketAddr = &SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 8000);

pub async fn serve_axum(app_state: AppState) -> Result<()> {
    let router = create_axum_router(app_state);
    tracing::debug!("listening on {}", ADDRESS);
    axum::Server::bind(ADDRESS)
        .serve(router.into_make_service())
        .await?;
    Ok(())
}

pub async fn serve_tonic(sender: Sender<models::DownloadGroup>) -> Result<()> {
    create_tonic_router(sender).serve(*ADDRESS).await?;
    Ok(())
}

pub async fn serve_combined(app_state: AppState) -> Result<()> {
    let tonic_router = create_tonic_router(app_state.downloads_channel.clone());
    let axum_router = create_axum_router(app_state)
        .map_result(|r| Result::<_, Box<dyn std::error::Error + Send + Sync>>::Ok(r?))
        .boxed_clone();

    let tonic_router = tonic_router
        .into_service()
        .map_response(|r| r.map(axum::body::boxed))
        .boxed_clone();

    let http_grpc = Steer::new(
        vec![axum_router, tonic_router],
        |req: &Request<Body>, _svcs: &[_]| {
            if req.headers().get(CONTENT_TYPE).map(|v| v.as_bytes()) != Some(b"application/grpc") {
                0
            } else {
                1
            }
        },
    );
    let binding = axum::Server::bind(ADDRESS).serve(Shared::new(http_grpc));
    info!("Listening on {ADDRESS}");
    binding.await?;
    Ok(())
}

pub fn create_axum_router(app_state: AppState) -> AxumRouter {
    let compression_predicate =
        DefaultPredicate::new().and(NotForContentType::const_new("text/event-stream"));

    AxumRouter::new()
        .nest("/", unversioned_routes())
        .nest("/v1", v1_routes())
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new().compress_when(compression_predicate))
                .layer(DecompressionLayer::new()),
        )
}

pub fn unversioned_routes() -> AxumRouter<AppState> {
    AxumRouter::new()
        .route("/series", get(controllers::rest::unversioned::find_anime))
        .route(
            "/series/:id",
            get(controllers::rest::unversioned::anime_by_id),
        )
        // Deprecated
        .route(
            "/downloads",
            get(controllers::rest::unversioned::find_downloads),
        )
        .route(
            "/downloads/updates",
            get(controllers::rest::unversioned::get_downloads_events),
        )
}

pub fn v1_routes() -> AxumRouter<AppState> {
    AxumRouter::new()
        .route(
            "/downloads/batches",
            get(controllers::rest::batch::find_downloads),
        )
        .route(
            "/downloads/episodes",
            get(controllers::rest::episode::find_downloads),
        )
        .route(
            "/downloads/episodes/updates",
            get(controllers::rest::episode::get_downloads_events),
        )
        .route(
            "/downloads/movies",
            get(controllers::rest::movie::find_downloads),
        )
}

pub fn create_tonic_router(sender: Sender<models::DownloadGroup>) -> TonicRouter {
    use proto::api::v1::downloads_server::DownloadsServer;
    let svc = DownloadsServer::new(controllers::grpc::DownloadService { sender });
    tonic::transport::Server::builder().add_service(svc)
}
