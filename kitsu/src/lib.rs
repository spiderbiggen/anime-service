#[macro_use]
extern crate serde;

pub mod models;

use hyper::client::connect::Connect;
use hyper::http::{request, StatusCode};
use hyper::{Body, Uri};
use request::Request;
use serde::{de, Deserialize};
use std::borrow::Borrow;
use thiserror::Error as ThisError;
use url::Url;

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
    Http(#[from] hyper::http::Error),
    #[error(transparent)]
    Uri(#[from] hyper::http::uri::InvalidUri),
    #[error(transparent)]
    Request(#[from] hyper::Error),
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

fn build_request(uri: Uri) -> request::Builder {
    Request::builder()
        .header(ACCEPT_HEADER, JSON_API_TYPE)
        .header(CONTENT_TYPE_HEADER, JSON_API_TYPE)
        .method("GET")
        .uri(uri)
}

async fn get_document<T, C>(client: hyper::Client<C>, uri: Uri) -> Result<T>
where
    C: Connect + Clone + Send + Sync + 'static,
    for<'de> T: de::Deserialize<'de>,
{
    let request = build_request(uri);
    let response = client.request(request.body(Body::empty())?).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::Status(status));
    }
    let body = hyper::body::to_bytes(response.into_body()).await?;
    let document = serde_json::from_slice(body.borrow())?;
    return Ok(document);
}

pub(self) async fn get_resource<T, C>(client: hyper::Client<C>, uri: Uri) -> Result<Single<T>>
where
    C: Connect + Clone + Send + Sync + 'static,
    for<'de> T: de::Deserialize<'de>,
{
    Ok(get_document::<Single<T>, C>(client, uri).await?)
}

pub(self) async fn get_resources<T, C>(client: hyper::Client<C>, uri: Uri) -> Result<Collection<T>>
where
    C: Connect + Clone + Send + Sync + 'static,
    for<'de> T: de::Deserialize<'de>,
{
    Ok(get_document::<Collection<T>, C>(client, uri).await?)
}

pub mod anime {
    use crate::*;

    pub async fn single<C>(client: hyper::Client<C>, id: u32) -> Result<Single<models::Anime>>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        let uri = format!("https://kitsu.io/api/edge/anime/{}", id).parse()?;
        let anime = get_resource::<models::Anime, C>(client, uri).await?;
        return Ok(anime);
    }

    pub async fn collection<C>(client: hyper::Client<C>) -> Result<Collection<models::Anime>>
    where
        C: Connect + Clone + Send + Sync + 'static,
    {
        let uri = "https://kitsu.io/api/edge/anime/".parse()?;
        let anime = get_resources::<models::Anime, C>(client, uri).await?;
        return Ok(anime);
    }
}
