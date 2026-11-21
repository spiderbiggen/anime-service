//! TMDB TV show endpoints.

use serde::Deserialize;

use crate::serde_helpers::{optional_date, optional_string};
use crate::{Result, TmdbClient};

/// A genre of a show.
#[derive(Debug, Clone, Deserialize)]
pub struct Genre {
    /// The TMDB genre id.
    pub id: u32,
    /// The localized genre name.
    pub name: String,
}

/// The details of a TV show.
///
/// TMDB documents no field as required, so everything except `id` and `name` falls back to a
/// default when missing.
#[derive(Debug, Clone, Deserialize)]
pub struct TvDetails {
    /// The TMDB id of the show.
    pub id: u32,
    /// The localized name.
    pub name: String,
    /// The name in the original language.
    #[serde(default)]
    pub original_name: String,
    /// The synopsis. Empty when TMDB has none in the requested language.
    #[serde(default)]
    pub overview: String,
    /// The production status, such as `Returning Series` or `Ended`.
    #[serde(default, deserialize_with = "optional_string")]
    pub status: Option<String>,
    /// The date the first episode aired, when known.
    #[serde(default, deserialize_with = "optional_date")]
    pub first_air_date: Option<jiff::civil::Date>,
    /// The date the last episode aired, when known.
    #[serde(default, deserialize_with = "optional_date")]
    pub last_air_date: Option<jiff::civil::Date>,
    /// The number of episodes aired so far.
    #[serde(default)]
    pub number_of_episodes: Option<u32>,
    /// The number of seasons.
    #[serde(default)]
    pub number_of_seasons: Option<u32>,
    /// Whether the show is flagged as adult content.
    #[serde(default)]
    pub adult: bool,
    /// The show's genres.
    #[serde(default)]
    pub genres: Vec<Genre>,
    /// The ISO 3166-1 codes of the countries the show originates from.
    #[serde(default)]
    pub origin_country: Vec<String>,
    /// The ISO 639-1 code of the original language.
    #[serde(default)]
    pub original_language: String,
    /// The poster path, see [`crate::images::poster_url`].
    #[serde(default, deserialize_with = "optional_string")]
    pub poster_path: Option<String>,
    /// The backdrop path, see [`crate::images::backdrop_url`].
    #[serde(default, deserialize_with = "optional_string")]
    pub backdrop_path: Option<String>,
}

/// The ids of a show on other services.
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalIds {
    /// The TMDB id of the show.
    pub id: u32,
    /// The `IMDb` id, such as `tt1234567`.
    #[serde(default, deserialize_with = "optional_string")]
    pub imdb_id: Option<String>,
    /// The `TheTVDB` id, which Sonarr uses.
    #[serde(default)]
    pub tvdb_id: Option<u32>,
}

/// Fetches the details of a TV show.
///
/// `language` is a code such as `en-US` (TMDB's default) or `ja-JP`.
///
/// # Errors
///
/// Returns an error if the request fails, TMDB answers with a non-success status (404 when the
/// show does not exist), or the response cannot be parsed.
pub async fn details(client: &TmdbClient, id: u32, language: Option<&str>) -> Result<TvDetails> {
    let query: Vec<_> = language
        .map(|language| ("language", language.to_owned()))
        .into_iter()
        .collect();
    client.get(&format!("tv/{id}"), &query).await
}

/// Fetches the ids of a TV show on other services.
///
/// # Errors
///
/// Returns an error if the request fails, TMDB answers with a non-success status (404 when the
/// show does not exist), or the response cannot be parsed.
pub async fn external_ids(client: &TmdbClient, id: u32) -> Result<ExternalIds> {
    client.get(&format!("tv/{id}/external_ids"), &[]).await
}
