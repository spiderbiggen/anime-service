//! TMDB search endpoints.

use serde::Deserialize;

use crate::serde_helpers::{optional_date, optional_string};
use crate::{ListResponse, Result, TmdbClient};

/// Parameters for [`tv`].
#[derive(Debug, Clone, Default)]
pub struct TvSearchParams {
    /// The text to search for.
    pub query: String,
    /// Restrict results to shows whose first air date is in this year.
    pub first_air_date_year: Option<u16>,
    /// Restrict results to shows with an air date (first or episode) in this year.
    pub year: Option<u16>,
    /// The language of the returned text, such as `en-US` (TMDB's default) or `ja-JP`.
    pub language: Option<String>,
    /// The 1-based page to fetch.
    pub page: Option<u32>,
}

impl TvSearchParams {
    /// Creates parameters that search for `query` with everything else left at TMDB's defaults.
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }

    fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut query = vec![("query", self.query.clone())];
        if let Some(year) = self.first_air_date_year {
            query.push(("first_air_date_year", year.to_string()));
        }
        if let Some(year) = self.year {
            query.push(("year", year.to_string()));
        }
        if let Some(language) = &self.language {
            query.push(("language", language.clone()));
        }
        if let Some(page) = self.page {
            query.push(("page", page.to_string()));
        }
        query
    }
}

/// A TV show in search results.
///
/// TMDB documents no field as required, so everything except `id` and `name` falls back to a
/// default when missing.
#[derive(Debug, Clone, Deserialize)]
pub struct TvSearchResult {
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
    /// The poster path, see [`crate::images::poster_url`].
    #[serde(default, deserialize_with = "optional_string")]
    pub poster_path: Option<String>,
    /// The backdrop path, see [`crate::images::backdrop_url`].
    #[serde(default, deserialize_with = "optional_string")]
    pub backdrop_path: Option<String>,
    /// The date the first episode aired, when known.
    #[serde(default, deserialize_with = "optional_date")]
    pub first_air_date: Option<jiff::civil::Date>,
    /// The ids of the show's genres. Animation is 16.
    #[serde(default)]
    pub genre_ids: Vec<u32>,
    /// The ISO 3166-1 codes of the countries the show originates from.
    #[serde(default)]
    pub origin_country: Vec<String>,
    /// The ISO 639-1 code of the original language.
    #[serde(default)]
    pub original_language: String,
    /// Whether the show is flagged as adult content.
    #[serde(default)]
    pub adult: bool,
    /// TMDB's popularity score.
    #[serde(default)]
    pub popularity: f64,
    /// The average vote, from 0 to 10.
    #[serde(default)]
    pub vote_average: f64,
    /// The number of votes.
    #[serde(default)]
    pub vote_count: u32,
}

/// The genre id TMDB uses for animation.
pub const ANIMATION_GENRE_ID: u32 = 16;

impl TvSearchResult {
    /// Returns whether the show is tagged with the animation genre.
    #[must_use]
    pub fn is_animation(&self) -> bool {
        self.genre_ids.contains(&ANIMATION_GENRE_ID)
    }
}

/// Searches for TV shows.
///
/// # Errors
///
/// Returns an error if the request fails, TMDB answers with a non-success status, or the response
/// cannot be parsed.
pub async fn tv(
    client: &TmdbClient,
    params: &TvSearchParams,
) -> Result<ListResponse<TvSearchResult>> {
    client.get("search/tv", &params.to_query()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_only_contains_the_parameters_that_are_set() {
        let params = TvSearchParams::new("Frieren");
        assert_eq!(params.to_query(), vec![("query", "Frieren".to_owned())]);
    }

    #[test]
    fn query_contains_every_parameter() {
        let params = TvSearchParams {
            query: "Frieren".into(),
            first_air_date_year: Some(2023),
            year: Some(2024),
            language: Some("ja-JP".into()),
            page: Some(2),
        };
        assert_eq!(
            params.to_query(),
            vec![
                ("query", "Frieren".to_owned()),
                ("first_air_date_year", "2023".to_owned()),
                ("year", "2024".to_owned()),
                ("language", "ja-JP".to_owned()),
                ("page", "2".to_owned()),
            ]
        );
    }
}
