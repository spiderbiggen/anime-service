use std::env;

use axum::extract::FromRef;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use url::Url;

use crate::errors::InternalError;
use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    pub client: HyperClient,
    pub pool: DBPool,
    pub downloads_cache: RequestCache<Vec<DownloadGroup>>,
}

pub type HyperClient = hyper::Client<HttpsConnector<HttpConnector>>;

impl FromRef<AppState> for HyperClient {
    fn from_ref(input: &AppState) -> Self {
        input.client.clone()
    }
}

pub type DBPool = Pool<Postgres>;

impl FromRef<AppState> for DBPool {
    fn from_ref(input: &AppState) -> Self {
        input.pool.clone()
    }
}

impl FromRef<AppState> for RequestCache<Vec<DownloadGroup>> {
    fn from_ref(input: &AppState) -> Self {
        input.downloads_cache.clone()
    }
}

pub(crate) fn create_hyper_client() -> hyper::Client<HttpsConnector<HttpConnector>> {
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();
    hyper::Client::builder().build(https)
}

pub(crate) async fn create_db_pool() -> Result<DBPool, InternalError> {
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
        .connect(url.as_ref())
        .await?;
    Ok(pool)
}
