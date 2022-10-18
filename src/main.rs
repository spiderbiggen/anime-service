mod models;

use std::error::Error;
use std::net::SocketAddr;
use axum::{Json, Router, routing::get};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tracing::error;
use kitsu;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

// our router
    let app = Router::new()
        .route("/", get(root))
        .route("/foo", get(get_foo).post(post_foo))
        .route("/foo/bar", get(foo_bar));

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// which calls one of these handlers
async fn root() -> &'static str {
    "Hello, World!"
}

async fn get_foo() -> Response {
    let anime = get_anime().await;
    match anime {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => {
            error!("Failed to fetch anime [{}]: {}", 1, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": format!("{}", e)
            }))).into_response()
        }
    }
}

async fn post_foo() {}

async fn foo_bar() {}

async fn get_anime() -> Result<models::Show, Box<dyn Error>> {
    let client: kitsu::Client = kitsu::Client::default();
    let anime = client.get_anime(1).await?;
    Ok(anime.data.try_into()?)
}

