use std::ops::RangeInclusive;

use crate::{DownloadVariant, Episode, Error};

pub(crate) mod subs_please;

/// Represents a nyaa download
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct ParsedDownload<'source> {
    pub(crate) source: &'source str,
    pub(crate) title: &'source str,
    pub(crate) download_type: ParsedDownloadVariant<'source>,
    pub(crate) resolution: u16,
}

/// The type of download determined from the file name
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum ParsedDownloadVariant<'source> {
    Batch(RangeInclusive<u32>),
    Episode(ParsedEpisode<'source>),
    Movie,
}

/// Wrap the episode info
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct ParsedEpisode<'source> {
    number: u32,
    decimal: Option<u32>,
    version: Option<u32>,
    extra: Option<&'source str>,
}

impl<'s> ParsedDownload<'s> {
    pub(crate) fn try_from(value: &'s str) -> Result<Self, Error> {
        match subs_please::parse_filename(value) {
            Ok(d) => Ok(d),
            Err(err) => Err(Error::ParseTitle(err.to_string())),
        }
    }
}

impl From<ParsedDownloadVariant<'_>> for DownloadVariant {
    fn from(value: ParsedDownloadVariant<'_>) -> Self {
        match value {
            ParsedDownloadVariant::Batch(batch) => Self::Batch(batch),
            ParsedDownloadVariant::Episode(ep) => Self::Episode(Episode {
                episode: ep.number,
                version: ep.version,
                decimal: ep.decimal,
                extra: ep.extra.map(|s| s.to_string()),
            }),
            ParsedDownloadVariant::Movie => Self::Movie,
        }
    }
}
