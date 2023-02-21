use anyhow::Result;
use axum::extract::FromRef;
use chrono::Duration;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use url::Url;

use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    pub client: ReqwestClient,
    pub pool: DBPool,
    pub downloads_cache: RequestCache<Vec<DownloadGroup>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            client: create_reqwest_client(),
            pool: create_db_pool().unwrap(),
            downloads_cache: RequestCache::new(Duration::minutes(5)),
        }
    }
}

pub type ReqwestClient = reqwest::Client;

impl FromRef<AppState> for ReqwestClient {
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

pub(crate) fn create_reqwest_client() -> reqwest::Client {
    reqwest::Client::new()
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