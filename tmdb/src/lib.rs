//! A small client for the [TMDB](https://www.themoviedb.org) v3 API.
//!
//! Only what anime-service needs is covered: TV search ([`search::tv`]), TV details
//! ([`tv::details`]), seasons ([`tv::season`]) and external ids ([`tv::external_ids`]). Requests authenticate with an API
//! read access token sent as a bearer header.
//!
//! ```no_run
//! use tmdb::TmdbClient;
//! use tmdb::search::{self, TvSearchParams};
//!
//! # async fn example() -> tmdb::Result<()> {
//! let client = TmdbClient::new(reqwest::Client::new(), "read-access-token")?;
//! let page = search::tv(&client, &TvSearchParams::new("Frieren")).await?;
//! println!("{} results", page.total_results);
//! # Ok(())
//! # }
//! ```

use std::fmt;

use reqwest::StatusCode;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tracing::instrument;
use url::Url;

pub mod images;
pub mod search;
mod serde_helpers;
pub mod tv;

/// The base URL of the TMDB v3 API.
pub const DEFAULT_BASE_URL: &str = "https://api.themoviedb.org/3/";

/// Errors returned by the TMDB client.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The request failed, or the response body could not be parsed.
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    /// A request URL could not be built.
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),
    /// TMDB answered with a non-success status code.
    #[error("request failed with status code: {0}")]
    Status(StatusCode),
}

/// A specialized [`Result`] for TMDB operations.
pub type Result<T> = std::result::Result<T, Error>;

/// A page of results, as returned by TMDB's paginated endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct ListResponse<T> {
    /// The 1-based page number.
    pub page: u32,
    /// The results on this page.
    pub results: Vec<T>,
    /// The total number of pages.
    pub total_pages: u32,
    /// The total number of results across all pages.
    pub total_results: u32,
}

/// A client for the TMDB API.
///
/// Cloning is cheap: the underlying [`reqwest::Client`] is shared.
#[derive(Clone)]
pub struct TmdbClient {
    http: reqwest::Client,
    access_token: String,
    base_url: Url,
}

impl fmt::Debug for TmdbClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TmdbClient")
            .field("base_url", &self.base_url.as_str())
            .field("access_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl TmdbClient {
    /// Creates a client that talks to [`DEFAULT_BASE_URL`], reusing an existing HTTP client.
    ///
    /// `access_token` is the API read access token (not the v3 API key).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if the default base URL cannot be parsed.
    pub fn new(http: reqwest::Client, access_token: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http,
            access_token: access_token.into(),
            base_url: Url::parse(DEFAULT_BASE_URL)?,
        })
    }

    /// Replaces the base URL, for example to point at a local test server.
    ///
    /// The URL must end with a `/`, otherwise its last path segment is replaced when joining.
    #[must_use]
    pub fn with_base_url(mut self, base_url: Url) -> Self {
        self.base_url = base_url;
        self
    }

    fn build_url(&self, path: &str, query: &[(&str, String)]) -> Result<Url> {
        let mut url = self.base_url.join(path)?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        Ok(url)
    }

    #[instrument(level = "trace", skip(self, query))]
    async fn get<T>(&self, path: &str, query: &[(&str, String)]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let url = self.build_url(path, query)?;
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Status(status));
        }
        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> TmdbClient {
        TmdbClient::new(reqwest::Client::new(), "secret-token").expect("default URL is valid")
    }

    #[test]
    fn debug_does_not_leak_the_token() {
        let rendered = format!("{:?}", client());
        assert!(!rendered.contains("secret-token"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn build_url_joins_path_and_encodes_query() {
        let url = client()
            .build_url("search/tv", &[("query", "Sousou no Frieren".into())])
            .expect("url builds");
        assert_eq!(
            url.as_str(),
            "https://api.themoviedb.org/3/search/tv?query=Sousou+no+Frieren"
        );
    }

    #[test]
    fn build_url_without_query_has_no_question_mark() {
        let url = client().build_url("tv/42", &[]).expect("url builds");
        assert_eq!(url.as_str(), "https://api.themoviedb.org/3/tv/42");
    }

    #[test]
    fn with_base_url_overrides_the_default() {
        let base = Url::parse("http://localhost:9999/3/").expect("valid url");
        let url = client()
            .with_base_url(base)
            .build_url("tv/42", &[])
            .expect("url builds");
        assert_eq!(url.as_str(), "http://localhost:9999/3/tv/42");
    }
}
