use std::{borrow::Borrow, collections::HashMap};

use chrono::{DateTime, Utc};
use hyper::client::connect::Connect;
use hyper::http::StatusCode;
use regex::Regex;
use rss::{Channel, Item};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] hyper::http::Error),
    #[error(transparent)]
    Uri(#[from] hyper::http::uri::InvalidUri),
    #[error(transparent)]
    Request(#[from] hyper::Error),
    #[error(transparent)]
    ParseUrl(#[from] url::ParseError),
    #[error(transparent)]
    Rss(#[from] rss::Error),
    #[error(transparent)]
    Regex(#[from] regex::Error),
    #[error("request failed with status code: {0}")]
    Status(StatusCode),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub struct AnimeSource {
    pub(crate) key: String,
    pub(crate) category: Option<String>,
    pub(crate) filter: Option<String>,
    pub(crate) regex: Regex,
}

impl AnimeSource {
    fn new<K>(key: K, category: Option<K>, regex: K, filter: Option<K>) -> Result<Self>
    where
        K: Into<String>,
    {
        Ok(Self {
            key: key.into(),
            category: category.and_then(|c| Some(c.into())),
            regex: Regex::new(&regex.into())?,
            filter: filter.and_then(|f| Some(f.into())),
        })
    }

    fn map_anime(&self, items: Vec<Item>) -> Vec<NyaaEntry> {
        items
            .into_iter()
            .filter_map(|i| to_anime(i, &self.regex))
            .collect()
    }

    fn build_url(&self, title: &str) -> Result<Url> {
        let query: String = format!("{} {}", self.key, title);
        let mut filters: Vec<(&str, &str)> = vec![("q", &query)];
        if let Some(ref category) = self.category {
            filters.push(("c", category.as_str()));
        }
        if let Some(ref filter) = self.filter {
            filters.push(("f", filter.as_str()));
        }
        Ok(Url::parse_with_params(
            "https://nyaa.si/?page=rss",
            filters,
        )?)
    }
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
    pub comments: String,
    pub resolution: String,
    pub torrent: String,
    pub file_name: String,
    pub pub_date: DateTime<Utc>,
}

pub async fn groups<C>(client: hyper::Client<C>, title: &str) -> Result<Vec<AnimeDownloads>>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    let entries = downloads(client, title).await?;
    Ok(map_groups(entries))
}

pub async fn downloads<C>(client: hyper::Client<C>, title: &str) -> Result<Vec<NyaaEntry>>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    let source = AnimeSource::new(
        "[SubsPlease]",
        Some("1_2"),
        r"^\[.*?] (.*) - (\d+)(?:\.(\d+))?(?:[vV](\d+?))? \((\d+?p)\) \[.*?\].mkv",
        None,
    )?;
    let anime = get_anime_for::<&String, C>(client.clone(), &source, title).await?;
    Ok(anime)
}

async fn get_anime_for<S, C>(
    client: hyper::Client<C>,
    source: &AnimeSource,
    title: &str,
) -> Result<Vec<NyaaEntry>>
where
    C: Connect + Clone + Send + Sync + 'static,
    S: AsRef<str>,
{
    let url = source.build_url(title)?;
    let val = get_feed(client, &url).await?;
    Ok(source.map_anime(val.items))
}

async fn get_feed<C>(client: hyper::Client<C>, url: &Url) -> Result<Channel>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    let response = client.get(url.as_str().parse()?).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::Status(status));
    }
    let body = hyper::body::to_bytes(response.into_body()).await?;
    let channel = Channel::read_from(body.borrow())?;
    Ok(channel)
}

fn map_groups(entries: Vec<NyaaEntry>) -> Vec<AnimeDownloads> {
    let mut result_map = HashMap::<Episode, Vec<Download>>::with_capacity(entries.capacity() / 3);
    for entry in entries {
        let episode = Episode {
            title: entry.title,
            episode: entry.episode,
            decimal: entry.decimal,
            version: entry.version,
        };
        let download = Download {
            comments: entry.comments,
            resolution: entry.resolution,
            torrent: entry.torrent,
            file_name: entry.file_name,
            pub_date: entry.pub_date,
        };
        match result_map.get_mut(&episode) {
            Some(v) => v.push(download),
            None => {
                result_map.insert(episode, vec![download]);
            }
        }
    }

    result_map
        .into_iter()
        .map(|(k, v)| AnimeDownloads {
            episode: k,
            downloads: v,
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct TitleParts(
    String,
    String,
    String,
    Option<u32>,
    Option<u32>,
    Option<u32>,
);

impl TitleParts {
    fn from_string<S>(inp: Option<S>, regex: &Regex) -> Option<TitleParts>
    where
        S: Into<String>,
    {
        inp.map(|s| s.into())
            .as_ref()
            .and_then(|title| regex.captures(title))
            .and_then(|cap| {
                let episode: Option<u32> = cap.get(2).and_then(|a| a.as_str().parse::<u32>().ok());
                let decimal: Option<u32> = cap.get(3).and_then(|a| a.as_str().parse::<u32>().ok());
                let version: Option<u32> = cap.get(4).and_then(|a| a.as_str().parse::<u32>().ok());
                let resolution: String = cap.get(5)?.as_str().to_string();

                Some(TitleParts(
                    cap.get(0)?.as_str().to_string(),
                    cap.get(1)?.as_str().to_string(),
                    resolution,
                    episode,
                    decimal,
                    version,
                ))
            })
    }
}

fn to_anime(item: Item, regex: &Regex) -> Option<NyaaEntry> {
    let date = item
        .pub_date
        .and_then(|str| DateTime::parse_from_rfc2822(&str).ok())?;
    let link = item.link?;
    let comments: String = item.guid?.value;

    TitleParts::from_string(item.title, regex).map(
        |TitleParts(file_name, title, resolution, episode, decimal, version)| NyaaEntry {
            episode,
            decimal,
            comments,
            version,
            resolution,
            title,
            file_name,
            torrent: link,
            pub_date: date.with_timezone(&Utc),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_source() -> AnimeSource {
        AnimeSource::new(
            "[SubsPlease]",
            Some("1_2"),
            "^\\[.*?] (.*) - (\\d+)(?:\\.(\\d+))?(?:[vV](\\d+?))? \\((\\d+?p)\\) \\[.*?\\].mkv",
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_parse_anime_components_basic() {
        let input = "[_] Test Anime - 01 (1080p) [_].mkv";
        let expected = TitleParts(
            "[_] Test Anime - 01 (1080p) [_].mkv".into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            None,
            None,
        );
        let source = get_source();
        let result = TitleParts::from_string(Some(input), &source.regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_version_lower() {
        let input = "[_] Test Anime - 01v1 (1080p) [_].mkv";
        let expected = TitleParts(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            None,
            Some(1),
        );
        let source = get_source();
        let result = TitleParts::from_string(Some(input), &source.regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_version_upper() {
        let input = "[_] Test Anime - 01V1 (1080p) [_].mkv";
        let expected = TitleParts(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            None,
            Some(1),
        );
        let source = get_source();
        let result = TitleParts::from_string(Some(input), &source.regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_decimal() {
        let input = "[_] Test Anime - 01.1 (1080p) [_].mkv";
        let expected = TitleParts(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            Some(1),
            None,
        );
        let source = get_source();
        let result = TitleParts::from_string(Some(input), &source.regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_decimal_and_version() {
        let input = "[_] Test Anime - 01.1V1 (1080p) [_].mkv";
        let expected = TitleParts(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            Some(1),
            Some(1),
        );
        let source = get_source();
        let result = TitleParts::from_string(Some(input), &source.regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_dash_in_title() {
        let input = "[_] Test-Anime - 01.1V1 (1080p) [_].mkv";
        let expected = TitleParts(
            input.into(),
            "Test-Anime".into(),
            "1080p".into(),
            Some(1),
            Some(1),
            Some(1),
        );
        let source = get_source();
        let result = TitleParts::from_string(Some(input), &source.regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }
}
