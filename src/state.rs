use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::FromRef;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tmdb::TmdbClient;
use tokio::sync::broadcast;
use url::Url;

use crate::models::DownloadGroup;

#[derive(Debug, Clone)]
pub struct AppState {
    pub client: ReqwestClient,
    pub pool: DBPool,
    pub downloads_channel: broadcast::Sender<DownloadGroup>,
    pub tmdb: TmdbClient,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let (tx, _) = broadcast::channel(32);
        let client = create_http_client()?;
        let tmdb = create_tmdb_client(client.clone())?;
        Ok(Self {
            client,
            pool: create_db_pool()?,
            downloads_channel: tx,
            tmdb,
        })
    }
}

pub type ReqwestClient = reqwest::Client;

/// How long to wait for a connection to an upstream service.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a whole request to an upstream service may take, including reading the body.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Creates the HTTP client shared by every upstream service (Nyaa and TMDB).
///
/// Connection pools are per host, so sharing the client mostly buys one place to configure the
/// timeouts and the user agent.
fn create_http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?)
}

impl FromRef<AppState> for ReqwestClient {
    fn from_ref(input: &AppState) -> Self {
        input.client.clone()
    }
}

impl FromRef<AppState> for TmdbClient {
    fn from_ref(input: &AppState) -> Self {
        input.tmdb.clone()
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

// No `Debug` derive: this holds a secret.
#[derive(Deserialize)]
struct TmdbConfig {
    access_token: String,
}

fn create_tmdb_client(http: reqwest::Client) -> Result<TmdbClient> {
    let config: TmdbConfig = envy::prefixed("TMDB_")
        .from_env()
        .context("TMDB_ACCESS_TOKEN must be set to a TMDB API read access token")?;
    Ok(TmdbClient::new(http, config.access_token)?)
}
