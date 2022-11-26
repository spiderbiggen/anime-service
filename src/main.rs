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
use thiserror::Error as ThisError;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing::error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, ThisError)]
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

static HYPER: Lazy<hyper::Client<HttpsConnector<HttpConnector>>> = Lazy::new(|| {
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();
    hyper::Client::builder().build(https)
});

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Self::ParseIntError(_) => (StatusCode::BAD_REQUEST, "Failed to parse integer"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
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
        .route("/anime/:id", get(anime::get_single))
        .route("/downloads", get(downloads::get))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(DecompressionLayer::new())
                .layer(CompressionLayer::new()),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
