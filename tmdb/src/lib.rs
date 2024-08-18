extern crate core;

mod year;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Debug, Clone, Copy)]
pub enum Authorization<'key> {
    Bearer(&'key str),
    ApiKey(&'key str),
}

impl<'key> Authorization<'key> {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Authorization::Bearer(token) => token,
            Authorization::ApiKey(key) => key,
        }
    }
}

impl<'key> Deref for Authorization<'key> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Debug, Clone)]
pub struct Client<'auth> {
    auth: Authorization<'auth>,
    reqwest: reqwest::Client,
}

impl<'auth> Client<'auth> {
    #[must_use]
    pub fn new(auth: Authorization<'auth>, reqwest: reqwest::Client) -> Self {
        Client { auth, reqwest }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paging<T> {
    pub page: u32,
    pub total_results: u32,
    pub total_pages: u32,
    pub results: Vec<T>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The provided authorization token is invalid")]
    Unauthorized,
    #[error("The requested resource could not be found")]
    NotFound,
    // TODO hide implementation details
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),
    // TODO hide implementation details
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    DeserializationError(#[from] serde_json::Error),
}

async fn handle_response<T: DeserializeOwned>(res: reqwest::Response) -> Result<T, Error> {
    let response = res.error_for_status()?;
    let body = response.text().await?;
    match serde_json::from_str(&body) {
        Ok(response) => Ok(response),
        Err(e) => {
            tracing::trace!(body = body, "error deserializing response: {}", e);
            Err(Error::DeserializationError(e))
        }
    }
}

pub mod search {
    use crate::year::Year;
    use crate::{Client, Error, Paging};
    use serde::{Deserialize, Serialize};
    use url::Url;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SearchResult {
        adult: bool,
        backdrop_path: String,
        genre_ids: Vec<u32>,
        id: u32,
        origin_country: Vec<String>,
        original_language: String,
        original_name: String,
        overview: String,
        popularity: f64,
        poster_path: String,
        first_air_date_year: Option<Year>,
        name: String,
        vote_average: f64,
        vote_count: u32,
    }

    // TODO Builder
    #[derive(Debug, Clone, Default)]
    pub struct Params<S: AsRef<str> = String> {
        pub first_air_date_year: Option<Year>,
        pub include_adult: Option<bool>,
        pub language: Option<S>,
        pub page: Option<u32>,
        pub year: Option<Year>,
    }

    impl<S: AsRef<str>> Params<S> {
        fn append_query_params(&self, url: &mut Url) {
            let mut pairs = url.query_pairs_mut();
            if let Some(year) = &self.first_air_date_year {
                pairs.append_pair("first_air_date_year", &year.to_string());
            }
            if let Some(include_adult) = &self.include_adult {
                pairs.append_pair("include_adult", &include_adult.to_string());
            }
            if let Some(language) = &self.language {
                pairs.append_pair("language", language.as_ref());
            }
            if let Some(page) = &self.page {
                pairs.append_pair("page", &page.to_string());
            }
            if let Some(year) = &self.year {
                pairs.append_pair("year", &year.to_string());
            }
        }
    }

    /// Search for TV shows based on a query.
    ///
    /// # Errors
    /// [`Error::Unauthorized`]: If the client is not authorized to make the request.
    /// [`Error::NotFound`]: If the resource requested does not exist.
    pub async fn tv<S>(
        client: &Client<'_>,
        query: S,
        params: Option<&Params>,
    ) -> Result<Paging<SearchResult>, Error>
    where
        S: AsRef<str>,
    {
        let mut url: Url = "https://api.themoviedb.org/3/search/tv".parse()?;
        url.query_pairs_mut().append_pair("query", query.as_ref());
        if let Some(params) = params {
            params.append_query_params(&mut url);
        }
        let response = client
            .reqwest
            .get(url)
            .bearer_auth(client.auth.as_str())
            .send()
            .await?;

        crate::handle_response(response).await
    }
}

pub mod tv {
    use crate::{Client, Error};
    use chrono::NaiveDate;
    use serde::{Deserialize, Serialize};
    use smallvec::SmallVec;
    use url::Url;

    #[derive(Serialize, Deserialize)]
    pub struct Author {
        pub id: u32,
        pub credit_id: u32,
        pub name: String,
        pub gender: u8,
        pub profile_path: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct SpokenLanguage {
        pub english_name: String,
        pub iso_639_1: String,
        pub name: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Season {
        pub air_date: String,
        pub episode_count: u32,
        pub id: u32,
        pub name: String,
        pub overview: String,
        pub poster_path: String,
        pub season_number: u32,
        pub vote_average: f64,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Country {
        pub iso_3166_1: String,
        pub name: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Network {
        pub id: u32,
        pub logo_path: String,
        pub name: String,
        pub origin_country: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Episode {
        pub id: u32,
        pub name: String,
        pub overview: String,
        pub vote_average: f64,
        pub vote_count: u32,
        pub air_date: String,
        pub episode_number: u32,
        pub episode_type: String,
        pub production_code: String,
        pub runtime: u32,
        pub season_number: u32,
        pub show_id: u32,
        pub still_path: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Genre {
        pub id: i64,
        pub name: String,
    }

    #[derive(Serialize, Deserialize)]
    pub struct Details {
        pub adult: bool,
        pub backdrop_path: String,
        pub created_by: Vec<Author>,
        pub episode_run_time: Vec<u32>,
        pub first_air_date: String,
        pub genres: Vec<Genre>,
        pub homepage: String,
        pub id: u32,
        pub in_production: bool,
        pub languages: Vec<String>,
        pub last_air_date: Option<NaiveDate>,
        pub last_episode_to_air: Option<Episode>,
        pub name: String,
        pub next_episode_to_air: Option<Episode>,
        pub networks: Vec<Network>,
        pub number_of_episodes: u32,
        pub number_of_seasons: u32,
        pub origin_country: Vec<String>,
        pub original_language: String,
        pub original_name: String,
        pub overview: String,
        pub popularity: f64,
        pub poster_path: String,
        pub production_companies: Vec<Network>,
        pub production_countries: Vec<Country>,
        pub seasons: Vec<Season>,
        pub spoken_languages: Vec<SpokenLanguage>,
        pub status: String,
        pub tagline: String,
        #[serde(rename = "type")]
        pub r#type: String,
        pub vote_average: f64,
        pub vote_count: u32,
    }

    // TODO Builder
    #[derive(Debug, Clone, Default)]
    pub struct Params<S: AsRef<str> = String> {
        pub append_to_response: SmallVec<[Endpoint; 20]>,
        pub language: Option<S>,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum Endpoint {
        AccountStates,
        AggregateCredits,
        AlternativeTitles,
        Changes,
        ContentRatings,
        Credits,
        EpisodeGroups,
        ExternalIDs,
        Images,
        Keywords,
        Latest,
        Lists,
        Recommendations,
        Review,
        ScreenedTheatrically,
        Similar,
        Translations,
        Videos,
        WatchProviders,
    }

    impl<S: AsRef<str>> Params<S> {
        fn append_query_params(&self, url: &mut Url) {
            let mut pairs = url.query_pairs_mut();
            if !self.append_to_response.is_empty() {
                // TODO
            }
            if let Some(language) = &self.language {
                pairs.append_pair("language", language.as_ref());
            }
        }
    }

    /// Search for TV shows based on a query.
    ///
    /// # Errors
    /// [`Error::Unauthorized`]: If the client is not authorized to make the request.
    /// [`Error::NotFound`]: If the resource requested does not exist.
    /// [`Error::DeserializationError`]: If resource does not match the expected format.
    pub async fn details(
        client: &Client<'_>,
        id: u32,
        params: Option<Params>,
    ) -> Result<Details, Error> {
        let url: Url = "https://api.themoviedb.org/3/tv/".parse()?;
        let mut url = url.join(&id.to_string()).expect("this should never fail");
        if let Some(params) = params {
            params.append_query_params(&mut url);
        }
        tracing::debug!("url: {}", url);
        let response = client
            .reqwest
            .get(url)
            .bearer_auth(client.auth.as_str())
            .send()
            .await?;

        crate::handle_response(response).await
    }
}
