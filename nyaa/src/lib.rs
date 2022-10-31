use std::borrow::Borrow;
use chrono::{DateTime, Utc};
use regex::Regex;
use rss::{Channel, Item};
use url::Url;
use futures::future::{join_all};
use hyper::Body;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

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
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub struct AnimeSource {
    pub(crate) key: String,
    pub(crate) category: Option<String>,
    pub(crate) filter: Option<String>,
    pub(crate) regex: Regex,
    pub(crate) resolutions: Vec<String>,
}

impl AnimeSource {
    fn new<K>(key: K, category: Option<K>, regex: K, filter: Option<K>, resolutions: Vec<K>) -> Result<Self>
        where K: Into<String>,
    {
        Ok(
            Self {
                key: key.into(),
                category: category.and_then(|c| Some(c.into())),
                regex: Regex::new(regex.into().as_str())?,
                filter: filter.and_then(|f| Some(f.into())),
                resolutions: resolutions.into_iter().map(|a| a.into()).collect(),
            }
        )
    }

    fn map_anime(&self, items: Vec<Item>) -> Vec<NyaaEntry> {
        items
            .into_iter()
            .filter_map(|i| to_anime(i, &self.regex))
            .collect()
    }

    fn build_url<S>(&self, res: S) -> Result<Url>
        where S: AsRef<str>
    {
        let query: String = format!("{key} {res}", key = self.key, res = res.as_ref());
        let mut filters: Vec<(&str, &str)> = vec![("q", &query)];
        if let Some(ref category) = self.category {
            filters.push(("c", category.as_str()));
        }
        if let Some(ref filter) = self.filter {
            filters.push(("f", filter.as_str()));
        }
        Ok(Url::parse_with_params("https://nyaa.si/?page=rss", filters)?)
    }
}

pub fn get_sources() -> Result<Vec<AnimeSource>> {
    Ok(
        vec![
            AnimeSource::new(
                "[SubsPlease]",
                Some("1_2"),
                "^\\[.*?] (.*) - (\\d+)(?:\\.(\\d+))?(?:[vV](\\d+?))? \\((\\d+?p)\\) \\[.*?\\].mkv",
                None,
                vec!["(1080p)", "(720p)", "(480p)"],
            )?,
        ]
    )
}

pub struct AnimeDownloads {
    pub episode: Episode,
    pub downloads: Vec<Download>,
}

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

pub struct Client {
    pub hyper: hyper::Client<HttpsConnector<HttpConnector>, Body>,
}

impl Default for Client {
    fn default() -> Self {
        Self { hyper: hyper::Client::builder().build(HttpsConnector::new()) }
    }
}

impl Client {
    pub fn new(hyper: hyper::Client<HttpsConnector<HttpConnector>, Body>) -> Self {
        Self { hyper }
    }

    pub async fn get_anime(&self) -> Result<Vec<NyaaEntry>> {
        let sources = get_sources()?;
        let tasks: Vec<_> = sources.iter()
            .flat_map(|source| source.resolutions.iter().map(|resolution| (source, resolution)).collect::<Vec<(&AnimeSource, &String)>>())
            .map(|(source, resolution)| self.get_anime_for::<&String>(source, resolution))
            .collect();
        let result = join_all(tasks).await;
        let result = result.into_iter().filter_map(|a| a.ok())
            .flatten()
            .collect();
        Ok(result)
    }

    async fn get_anime_for<S>(&self, source: &AnimeSource, resolution: &String) -> Result<Vec<NyaaEntry>>
        where S: AsRef<str>
    {
        let url = source.build_url(resolution)?;
        let val = self.get_feed(&url).await?;
        Ok(source.map_anime(val.items))
    }

    async fn get_feed(&self, url: &Url) -> Result<Channel> {
        let response = self.hyper.get(url.as_str().parse()?).await?;
        let body = hyper::body::to_bytes(response.into_body()).await?;
        let channel = Channel::read_from(body.borrow())?;
        Ok(channel)
    }
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
        inp.and_then(|s| Some(s.into()))
            .as_ref()
            .and_then(|title| regex.captures(title))
            .and_then(|cap| {
                let episode: Option<u32> = cap.get(2).and_then(|a| a.as_str().parse::<u32>().ok());
                let decimal: Option<u32> = cap.get(3).and_then(|a| a.as_str().parse::<u32>().ok());
                let version: Option<u32> = cap.get(4).and_then(|a| a.as_str().parse::<u32>().ok());
                let resolution: String = cap.get(5).unwrap().as_str().to_string();

                Some(TitleParts(
                    cap[0].into(),
                    cap[1].into(),
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
        .as_ref()
        .and_then(|str| DateTime::parse_from_rfc2822(str).ok())?;
    let link = item.link?;
    let comments: String = item.guid?.value;

    TitleParts::from_string(item.title, regex).and_then(
        |TitleParts(file_name, title, resolution, episode, decimal, version)| {
            Some(NyaaEntry {
                episode,
                decimal,
                comments,
                version,
                resolution,
                title,
                file_name,
                torrent: link,
                pub_date: date.with_timezone(&Utc),
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let source = get_sources().unwrap().get(0).unwrap().clone();
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
        let source = get_sources().unwrap().get(0).unwrap().clone();
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
        let source = get_sources().unwrap().get(0).unwrap().clone();
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
        let source = get_sources().unwrap().get(0).unwrap().clone();
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
        let source = get_sources().unwrap().get(0).unwrap().clone();
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
        let source = get_sources().unwrap().get(0).unwrap().clone();
        let result = TitleParts::from_string(Some(input), &source.regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }
}
