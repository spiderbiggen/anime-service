mod controllers;
mod errors;
mod jobs;
mod models;
mod sql_models;

use crate::controllers::{anime, downloads};
use axum::extract::FromRef;
use axum::{routing::get, Router};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;

use crate::errors::InternalError;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::{env, net::SocketAddr};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, decompression::DecompressionLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

#[derive(Debug, Clone)]
struct AppState {
    client: HyperClient,
    pool: DBPool,
}

type HyperClient = hyper::Client<HttpsConnector<HttpConnector>>;

impl FromRef<AppState> for HyperClient {
    fn from_ref(input: &AppState) -> Self {
        input.client.clone()
    }
}

type DBPool = Pool<Postgres>;

impl FromRef<AppState> for DBPool {
    fn from_ref(input: &AppState) -> Self {
        input.pool.clone()
    }
}

fn create_hyper_client() -> hyper::Client<HttpsConnector<HttpConnector>> {
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();
    hyper::Client::builder().build(https)
}

async fn create_db_pool() -> Result<Pool<Postgres>, InternalError> {
    let mut url = Url::parse("postgres://")?;
    url.set_host(Some(&env::var("PG_HOST")?))?;
    url.set_password(env::var("PG_PASS").ok().as_deref())
        .expect("password should be accepted");
    if let Ok(u) = env::var("PG_USER") {
        url.set_username(&u).expect("password should be accepted");
    }
    if let Ok(port) = env::var("PG_PORT") {
        url.set_port(Some(port.parse::<u16>()?))
            .expect("port should be accepted");
    }
    url.set_path(&env::var("PG_DATABASE")?);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url.to_string())
        .await?;
    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<(), InternalError> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = create_hyper_client();
    let pool = create_db_pool().await?;

    sqlx::migrate!().run(&pool).await?;

    let poller_job = jobs::poller::start(client.clone(), pool.clone());
    let app_state = AppState { client, pool };
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
    tokio::try_join!(poller_job)?;
    Ok(())
}
