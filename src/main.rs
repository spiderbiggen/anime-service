mod controllers;
mod models;

use crate::controllers::{anime, downloads};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use once_cell::sync::Lazy;
use serde_json::json;
use std::{net::SocketAddr, num::ParseIntError};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing::error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    Kitsu(#[from] kitsu::Error),
    #[error(transparent)]
    Nyaa(#[from] nyaa::Error),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
}

type HyperClient = hyper::Client<HttpsConnector<HttpConnector>>;

fn create_hyper_client() -> hyper::Client<HttpsConnector<HttpConnector>> {
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();
    hyper::Client::builder().build(https)
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!("request failed with {self}");
        let (status, error_message) = match self {
            Self::Nyaa(nyaa::Error::Status(code)) => {
                (code, code.canonical_reason().unwrap_or_default())
            }
            Self::Kitsu(kitsu::Error::Status(code)) => {
                (code, code.canonical_reason().unwrap_or_default())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();
    // our router
    let app = Router::new()
        .route("/series", get(anime::get_collection))
        .route("/series/:id", get(anime::get_single))
        .route("/downloads", get(downloads::get))
        .with_state(create_hyper_client())
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
