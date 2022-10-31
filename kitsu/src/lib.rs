#[macro_use]
extern crate serde;

pub mod models;

use std::borrow::Borrow;
use hyper::{Body, Uri};
use hyper::client::HttpConnector;
use hyper::http::request;
use hyper_tls::HttpsConnector;
use request::Request;
use serde::{de, Deserialize};
use url::Url;
use thiserror::Error as ThisError;

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

pub struct Client {
    pub hyper: hyper::Client<HttpsConnector<HttpConnector>, Body>,
}

impl Client {
    pub fn new(hyper: hyper::Client<HttpsConnector<HttpConnector>, Body>) -> Self {
        Self { hyper }
    }

    fn build_request(&self, uri: Uri) -> request::Builder {
        Request::builder()
            .header(ACCEPT_HEADER, JSON_API_TYPE)
            .header(CONTENT_TYPE_HEADER, JSON_API_TYPE)
            .method("GET")
            .uri(uri)
    }

    async fn get_document<T>(&self, uri: Uri) -> Result<T>
        where for<'de> T: de::Deserialize<'de> {
        let request = self.build_request(uri);
        let response = self.hyper.request(request.body(Body::empty())?).await?;
        let body = hyper::body::to_bytes(response.into_body()).await?;
        let document = serde_json::from_slice(body.borrow())?;
        return Ok(document);
    }

    pub(self) async fn get_resource<T>(&self, uri: Uri) -> Result<Single<T>>
        where for<'de> T: de::Deserialize<'de> {
        let doc = self.get_document::<Single<T>>(uri).await?;
        Ok(doc)
    }

    pub(self) async fn get_resources<T>(&self, uri: Uri) -> Result<Collection<T>>
        where for<'de> T: de::Deserialize<'de> {
        let doc = self.get_document::<Collection<T>>(uri).await?;
        Ok(doc)
    }

    pub async fn get_anime(&self, id: u32) -> Result<Single<models::Anime>> {
        let uri = format!("https://kitsu.io/api/edge/anime/{}", id).parse()?;
        let anime = self.get_resource::<models::Anime>(uri).await?;
        return Ok(anime);
    }
}

impl Default for Client {
    fn default() -> Self {
        Self { hyper: hyper::Client::builder().build(HttpsConnector::new()) }
    }
}

