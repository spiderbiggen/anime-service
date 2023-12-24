use anyhow::Result;
use axum::extract::FromRef;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tokio::sync::broadcast;
use url::Url;

use crate::models::DownloadGroup;

#[derive(Debug, Clone)]
pub struct AppState {
    pub client: ReqwestClient,
    pub pool: DBPool,
    pub downloads_channel: broadcast::Sender<DownloadGroup>,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let (tx, _) = broadcast::channel(32);
        Ok(Self {
            client: reqwest::Client::new(),
            pool: create_db_pool()?,
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

pub fn create_db_pool() -> Result<DBPool> {
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
        .connect_lazy(url.as_ref())?;
    Ok(pool)
}
