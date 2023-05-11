use anyhow::Result;
use axum::extract::FromRef;
use chrono::Duration;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tokio::sync::broadcast;
use url::Url;

use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    pub client: ReqwestClient,
    pub pool: DBPool,
    pub downloads_cache: RequestCache<Vec<DownloadGroup>>,
    pub downloads_channel: broadcast::Sender<DownloadGroup>,
}

impl AppState {
    pub(crate) async fn new() -> Result<Self> {
        let (tx, _) = broadcast::channel(32);
        Ok(Self {
            client: reqwest::Client::new(),
            pool: create_db_pool().await?,
            downloads_cache: RequestCache::new(Duration::minutes(5)),
            downloads_channel: tx,
        })
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

impl FromRef<AppState> for broadcast::Sender<DownloadGroup> {
    fn from_ref(input: &AppState) -> Self {
        input.downloads_channel.clone()
    }
}

#[derive(Debug, Deserialize)]
struct DbConfig {
    host: String,
    port: u16,
    user: String,
    pass: String,
    database: String,
}

pub(crate) async fn create_db_pool() -> Result<DBPool> {
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

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(url.as_ref())
        .await?;
    Ok(pool)
}
