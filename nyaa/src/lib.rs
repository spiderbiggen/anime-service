use std::fmt::Debug;
use std::num::ParseIntError;
use std::ops::RangeInclusive;
use std::{borrow::Borrow, collections::HashMap};

use crate::parsers::ParsedDownload;
use ahash::RandomState;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use rss::{Channel, Item};
use tracing::instrument;
use url::Url;
use url_macro::url;

mod parsers;

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
    #[error("{0}")]
    ParseTitle(String),
    #[error(transparent)]
    Rss(#[from] rss::Error),
    #[error("request failed with status code: {0}")]
    Status(StatusCode),
    #[error("no {0} found")]
    None(&'static str),
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum DownloadVariant {
    Batch(RangeInclusive<u32>),
    Episode(Episode),
    Movie,
}

#[derive(Debug)]
pub struct AnimeDownloads {
    pub title: String,
    pub variant: DownloadVariant,
    pub downloads: Vec<Download>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Episode {
    pub episode: u32,
    pub decimal: Option<u32>,
    pub version: Option<u32>,
    pub extra: Option<String>,
}

#[derive(Debug)]
pub struct Download {
    pub comments: String,
    pub resolution: u16,
    pub torrent: String,
    pub file_name: String,
    pub pub_date: DateTime<Utc>,
}

#[derive(Debug)]
struct Entry {
    title: String,
    variant: DownloadVariant,
    download: Download,
}

#[instrument(skip(client))]
pub async fn groups(
    client: &reqwest::Client,
    title: Option<&str>,
) -> Result<impl Iterator<Item = AnimeDownloads>, Error> {
    let url = build_url(title);
    let val = get_feed(client, url).await?;
    let entries = val.items.into_iter().filter_map(|i| map_item(i).ok());
    Ok(map_groups(entries))
}

#[instrument(skip_all, fields(url = %url))]
async fn get_feed(client: &reqwest::Client, url: Url) -> Result<Channel, Error> {
    tracing::trace!("Requesting RSS feed");
    let response = client.get(url).send().await?;
    let status = response.status();
    tracing::trace!(
        "Got response with status: {:?} and length: {:?} bytes",
        status,
        response.content_length()
    );
    if !status.is_success() {
        return Err(Error::Status(status));
    }
    let body = response.bytes().await?;
    Ok(Channel::read_from(body.borrow())?)
}

#[must_use]
fn build_url(title: Option<&str>) -> Url {
    let mut url = url!("https://nyaa.si/");
    let mut query: String = String::from("[SubsPlease]");
    if let Some(title) = title {
        query.push(' ');
        query.push_str(title.as_ref());
    }
    url.query_pairs_mut()
        .append_pair("page", "rss")
        .append_pair("q", &query)
        .append_pair("c", "1_2")
        .append_pair("f", "2");
    url
}

#[instrument(err)]
fn map_item(item: Item) -> Result<Entry, Error> {
    let pub_date = item.pub_date.ok_or(Error::None("rss pub date"))?;
    let date = DateTime::parse_from_rfc2822(&pub_date)?;
    let file_name = item.title.ok_or(Error::None("rss title"))?;
    let parts = ParsedDownload::try_from(&file_name)?;

    Ok(Entry {
        title: parts.title.to_string(),
        variant: parts.download_type.into(),
        download: Download {
            comments: item.guid.ok_or(Error::None("rss guid"))?.value,
            resolution: parts.resolution,
            file_name,
            torrent: item.link.ok_or(Error::None("rss link"))?,
            pub_date: date.with_timezone(&Utc),
        },
    })
}

fn map_groups<I>(entries: I) -> impl Iterator<Item = AnimeDownloads>
where
    I: Iterator<Item = Entry>,
{
    let mut result_map = HashMap::<_, Vec<_>, RandomState>::default();
    for entry in entries {
        result_map
            .entry((entry.title, entry.variant))
            .or_default()
            .push(entry.download);
    }

    result_map
        .into_iter()
        .map(|((title, variant), downloads)| AnimeDownloads {
            title,
            variant,
            downloads,
        })
}
