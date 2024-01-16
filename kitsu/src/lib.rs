#[macro_use]
extern crate serde;

use reqwest::StatusCode;
use serde::{de, Deserialize};
use thiserror::Error as ThisError;
use tracing::instrument;
use url::Url;

pub mod models;

const JSON_API_TYPE: &str = "application/vnd.api+json";
const ACCEPT_HEADER: &str = "Accept";
const CONTENT_TYPE_HEADER: &str = "Content-Type";

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Empty Response")]
    Empty,
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),
    #[error("request failed with status code: {0}")]
    Status(StatusCode),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Meta {
    pub count: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Links {
    pub first: Option<Url>,
    pub next: Option<Url>,
    pub last: Option<Url>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Collection<T> {
    pub data: Vec<T>,
    pub meta: Meta,
    pub links: Links,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Single<T> {
    pub data: T,
}

fn build_request(client: reqwest::Client, url: Url) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header(ACCEPT_HEADER, JSON_API_TYPE)
        .header(CONTENT_TYPE_HEADER, JSON_API_TYPE)
}

async fn get_document<T>(client: reqwest::Client, url: Url) -> Result<T>
where
    for<'de> T: de::Deserialize<'de>,
{
    let request = build_request(client, url);
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::Status(status));
    }
    Ok(response.json().await?)
}

#[instrument(level = "trace", skip_all, fields(url = %url))]
async fn get_resource<T>(client: reqwest::Client, url: Url) -> Result<Single<T>>
where
    for<'de> T: de::Deserialize<'de>,
{
    get_document::<Single<T>>(client, url).await
}

#[instrument(level = "trace", skip_all, fields(url = %url))]
async fn get_resources<T>(client: reqwest::Client, url: Url) -> Result<Collection<T>>
where
    for<'de> T: de::Deserialize<'de>,
{
    get_document::<Collection<T>>(client, url).await
}

pub mod anime {
    use crate::{get_resource, get_resources, models};
    use crate::{Collection, Result, Single};
    use url::Url;

    pub async fn single(client: reqwest::Client, id: u32) -> Result<Single<models::Anime>> {
        let url: Url = "https://kitsu.io/api/edge/anime/".parse()?;
        let url = url.join(&id.to_string())?;
        let anime = get_resource::<models::Anime>(client, url).await?;
        Ok(anime)
    }

    pub async fn collection(client: reqwest::Client) -> Result<Collection<models::Anime>> {
        let uri = Url::parse("https://kitsu.io/api/edge/anime/")?;
        let anime = get_resources::<models::Anime>(client, uri).await?;
        Ok(anime)
    }
}
