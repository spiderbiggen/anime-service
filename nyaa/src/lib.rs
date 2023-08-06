use std::fmt::Debug;
use std::num::ParseIntError;
use std::{borrow::Borrow, collections::HashMap};

use chrono::{DateTime, Utc};
use regex::{Captures, Regex};
use reqwest::StatusCode;
use rss::{Channel, Item};
use tracing::{error, instrument, trace};
use url::Url;

const SUBS_PLEASE_REGEX: &str = r"^\[.*?] (?<title>.*) - (?<episode>\d+)(?:\.(?<decimal>\d+))?(?:[vV](?<version>\d+?))?(?<extra>[a-zA-Z]*) \((?<resolution>\d+?p)\) \[.*?\].mkv";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    ParseUrl(#[from] url::ParseError),
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
    #[error(transparent)]
    ParseDate(#[from] chrono::ParseError),
    #[error(transparent)]
    Rss(#[from] rss::Error),
    #[error(transparent)]
    Regex(#[from] regex::Error),
    #[error("request failed with status code: {0}")]
    Status(StatusCode),
    #[error(transparent)]
    DeserializeError(#[from] DeserializeError),
    #[error("no {0} found")]
    None(&'static str),
}

#[derive(Debug, thiserror::Error)]
pub enum DeserializeError {
    #[error(transparent)]
    ParseUrl(#[from] url::ParseError),
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
    #[error(transparent)]
    ParseDate(#[from] chrono::ParseError),
    #[error(transparent)]
    Rss(#[from] rss::Error),
    #[error("no {0} found")]
    None(&'static str),
    #[error("{file_name} didn't match {matcher}")]
    NoMatches { file_name: String, matcher: Regex },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Debug)]
pub struct AnimeSource {
    pub(crate) key: String,
    pub(crate) category: Option<String>,
    pub(crate) regex: Regex,
}

impl AnimeSource {
    fn new<K, C, R>(key: K, category: Option<C>, regex: R) -> Result<Self>
    where
        K: Into<String>,
        C: Into<String>,
        R: AsRef<str>,
    {
        Ok(Self {
            key: key.into(),
            category: category.map(|c| c.into()),
            regex: Regex::new(regex.as_ref())?,
        })
    }

    fn build_url<S>(&self, title: Option<S>) -> Result<Url>
    where
        S: AsRef<str>,
    {
        let mut query: String = self.key.clone();
        if let Some(title) = title {
            query.push(' ');
            query.push_str(title.as_ref());
        }
        let mut params: Vec<(&str, &str)> = vec![("q", &query)];
        if let Some(ref category) = self.category {
            params.push(("c", category.as_str()));
        }
        Ok(Url::parse_with_params("https://nyaa.si/?page=rss", params)?)
    }
}

fn map_anime(items: Vec<Item>, title_matcher: &Regex) -> Vec<NyaaEntry> {
    items
        .into_iter()
        .filter_map(|i| map_item(i, title_matcher).ok())
        .collect()
}

#[instrument(err)]
fn map_item(item: Item, title_matcher: &Regex) -> Result<NyaaEntry> {
    let pub_date = item.pub_date.ok_or(Error::None("rss pub date"))?;
    let date = DateTime::parse_from_rfc2822(&pub_date)?;
    let file_name = item.title.ok_or(Error::None("rss title"))?;
    let parts = TitleParts::from_string(&file_name, title_matcher)?;

    Ok(NyaaEntry {
        file_name,
        episode: parts.episode,
        decimal: parts.decimal,
        comments: item.guid.ok_or(Error::None("rss guid"))?.value,
        version: parts.version,
        extra: parts.extra,
        resolution: parts.resolution,
        title: parts.title,
        torrent: item.link.ok_or(Error::None("rss link"))?,
        pub_date: date.with_timezone(&Utc),
    })
}

pub struct AnimeDownloads {
    pub episode: Episode,
    pub downloads: Vec<Download>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Episode {
    pub title: String,
    pub episode: Option<u32>,
    pub decimal: Option<u32>,
    pub version: Option<u32>,
    pub extra: Option<String>,
}

pub struct Download {
    pub comments: String,
    pub resolution: String,
    pub torrent: String,
    pub file_name: String,
    pub pub_date: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NyaaEntry {
    pub title: String,
    pub episode: Option<u32>,
    pub decimal: Option<u32>,
    pub version: Option<u32>,
    pub extra: Option<String>,
    pub comments: String,
    pub resolution: String,
    pub torrent: String,
    pub file_name: String,
    pub pub_date: DateTime<Utc>,
}

pub async fn groups(client: reqwest::Client, title: Option<&str>) -> Result<Vec<AnimeDownloads>> {
    let entries = downloads(client, title).await?;
    Ok(map_groups(entries))
}

pub async fn downloads(client: reqwest::Client, title: Option<&str>) -> Result<Vec<NyaaEntry>> {
    let source = AnimeSource::new("[SubsPlease]", Some("1_2"), SUBS_PLEASE_REGEX)?;
    get_anime_for(client.clone(), &source, title).await
}

async fn get_anime_for(
    client: reqwest::Client,
    source: &AnimeSource,
    title: Option<&str>,
) -> Result<Vec<NyaaEntry>> {
    let url = source.build_url(title)?;
    let val = get_feed(client, url).await?;
    Ok(map_anime(val.items, &source.regex))
}

async fn get_feed(client: reqwest::Client, url: Url) -> Result<Channel> {
    let response = client.get(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::Status(status));
    }
    let body = response.bytes().await?;
    let channel = Channel::read_from(body.borrow())?;
    Ok(channel)
}

fn map_groups(entries: Vec<NyaaEntry>) -> Vec<AnimeDownloads> {
    let groups_estimate = (entries.len() / 3) + 1;
    let mut result_map = HashMap::<Episode, Vec<Download>>::with_capacity(groups_estimate);
    for entry in entries {
        let episode = Episode {
            title: entry.title,
            episode: entry.episode,
            decimal: entry.decimal,
            version: entry.version,
            extra: entry.extra,
        };
        let download = Download {
            comments: entry.comments,
            resolution: entry.resolution,
            torrent: entry.torrent,
            file_name: entry.file_name,
            pub_date: entry.pub_date,
        };
        result_map
            .entry(episode)
            .or_insert_with(|| Vec::with_capacity(3))
            .push(download);
    }

    result_map
        .into_iter()
        .map(|(episode, downloads)| AnimeDownloads { episode, downloads })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct TitleParts {
    title: String,
    resolution: String,
    episode: Option<u32>,
    decimal: Option<u32>,
    version: Option<u32>,
    extra: Option<String>,
}

impl TitleParts {
    #[instrument(err)]
    fn from_string(file_name: &str, regex: &Regex) -> Result<TitleParts, DeserializeError> {
        let cap = regex
            .captures(file_name)
            .ok_or(DeserializeError::None("captures"))?;
        let title = string_named_group(&cap, "title").ok_or(DeserializeError::None("title"))?;
        let resolution =
            string_named_group(&cap, "resolution").ok_or(DeserializeError::None("resolution"))?;
        let episode = int_named_group(&cap, "episode")?;
        let decimal = int_named_group(&cap, "decimal")?;
        let version = int_named_group(&cap, "version")?;
        let extra = string_named_group(&cap, "extra");

        Ok(TitleParts {
            title,
            resolution,
            episode,
            decimal,
            version,
            extra,
        })
    }
}

fn string_named_group(captures: &Captures, name: &str) -> Option<String> {
    match captures.name(name).map(|s| s.as_str())? {
        "" => None,
        s => Some(s.into()),
    }
}

fn int_named_group(captures: &Captures, name: &str) -> Result<Option<u32>, ParseIntError> {
    match captures.name(name).map(|s| s.as_str().parse::<u32>()) {
        Some(Ok(num)) => Ok(Some(num)),
        Some(Err(e)) => Err(e)?,
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_source() -> AnimeSource {
        AnimeSource::new("[SubsPlease]", Some("1_2"), SUBS_PLEASE_REGEX).unwrap()
    }

    #[test]
    fn test_parse_anime_components_basic() {
        let input = "[_] Test Anime - 01 (1080p) [_].mkv";
        let expected = TitleParts {
            title: "Test Anime".into(),
            resolution: "1080p".into(),
            episode: Some(1),
            decimal: None,
            version: None,
            extra: None,
        };
        let source = get_source();
        let result = TitleParts::from_string(input.into(), &source.regex).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_anime_components_with_version_lower() {
        let input = "[_] Test Anime - 01v1 (1080p) [_].mkv";
        let expected = TitleParts {
            title: "Test Anime".into(),
            resolution: "1080p".into(),
            episode: Some(1),
            decimal: None,
            version: Some(1),
            extra: None,
        };
        let source = get_source();
        let result = TitleParts::from_string(input.into(), &source.regex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_version_upper() {
        let input = "[_] Test Anime - 01V1 (1080p) [_].mkv";
        let expected = TitleParts {
            title: "Test Anime".into(),
            resolution: "1080p".into(),
            episode: Some(1),
            decimal: None,
            version: Some(1),
            extra: None,
        };
        let source = get_source();
        let result = TitleParts::from_string(input.into(), &source.regex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_decimal() {
        let input = "[_] Test Anime - 01.1 (1080p) [_].mkv";
        let expected = TitleParts {
            title: "Test Anime".into(),
            resolution: "1080p".into(),
            episode: Some(1),
            decimal: Some(1),
            version: None,
            extra: None,
        };
        let source = get_source();
        let result = TitleParts::from_string(input.into(), &source.regex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_decimal_and_version() {
        let input = "[_] Test Anime - 01.1V1 (1080p) [_].mkv";
        let expected = TitleParts {
            title: "Test Anime".into(),
            resolution: "1080p".into(),
            episode: Some(1),
            decimal: Some(1),
            version: Some(1),
            extra: None,
        };
        let source = get_source();
        let result = TitleParts::from_string(input.into(), &source.regex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_dash_in_title() {
        let input = "[_] Test-Anime - 01.1V1 (1080p) [_].mkv";
        let expected = TitleParts {
            title: "Test-Anime".into(),
            resolution: "1080p".into(),
            episode: Some(1),
            decimal: Some(1),
            version: Some(1),
            extra: None,
        };
        let source = get_source();
        let result = TitleParts::from_string(input.into(), &source.regex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }
    #[test]
    fn test_parse_anime_components_with_extras_in_episode_number() {
        let input = "[_] Test-Anime - 1D (1080p) [_].mkv";
        let expected = TitleParts {
            title: "Test-Anime".into(),
            resolution: "1080p".into(),
            episode: Some(1),
            decimal: None,
            version: None,
            extra: Some("D".into()),
        };
        let source = get_source();
        let result = TitleParts::from_string(input.into(), &source.regex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }
}
