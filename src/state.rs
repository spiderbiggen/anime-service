use anyhow::Result;
use axum::extract::FromRef;
use chrono::Duration;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use url::Url;

use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    pub client: HyperClient,
    pub pool: DBPool,
    pub downloads_cache: RequestCache<Vec<DownloadGroup>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            client: create_hyper_client(),
            pool: create_db_pool().unwrap(),
            downloads_cache: RequestCache::new(Duration::minutes(5)),
        }
    }
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

#[derive(Debug, Deserialize)]
struct DbConfig {
    host: String,
    port: u16,
    user: String,
    pass: String,
    database: String,
}

pub(crate) fn create_db_pool() -> Result<DBPool> {
    let config: DbConfig = envy::prefixed("PG_").from_env()?;

    let mut url = Url::parse("postgres://")?;
    url.set_host(Some(&config.host))?;
    url.set_password(Some(&config.pass))
        .expect("password should be accepted");
    url.set_username(&config.user)
        .expect("username should be accepted");
    url.set_port(Some(config.port))
        .expect("port should be accepted");
    url.set_path(&config.database);

    Ok(PgPoolOptions::new().connect_lazy(url.as_ref())?)
}
