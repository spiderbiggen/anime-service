use std::num::ParseIntError;
use std::ops::RangeInclusive;

use chrono::{DateTime, Utc};
use serde::Serialize;
use url::Url;

use kitsu::models as kitsu;

#[derive(Serialize, Copy, Clone, Debug)]
pub struct ImageDimension {
    pub width: u32,
    pub height: u32,
}

impl From<kitsu::ImageDimension> for ImageDimension {
    fn from(value: kitsu::ImageDimension) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ImageDefinition {
    pub url: Url,
    pub dimensions: ImageDimension,
}

#[derive(Serialize, Clone, Debug)]
pub struct Images {
    pub original: Url,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large: Option<ImageDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<ImageDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small: Option<ImageDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiny: Option<ImageDefinition>,
}

impl From<kitsu::Images> for Images {
    fn from(value: kitsu::Images) -> Self {
        Self {
            original: value.original,
            large: value
                .large
                .zip(value.meta.dimensions.large)
                .map(|(u, i)| ImageDefinition {
                    url: u,
                    dimensions: i.into(),
                }),
            medium: value
                .medium
                .zip(value.meta.dimensions.medium)
                .map(|(u, i)| ImageDefinition {
                    url: u,
                    dimensions: i.into(),
                }),
            small: value
                .small
                .zip(value.meta.dimensions.small)
                .map(|(u, i)| ImageDefinition {
                    url: u,
                    dimensions: i.into(),
                }),
            tiny: value
                .tiny
                .zip(value.meta.dimensions.tiny)
                .map(|(u, i)| ImageDefinition {
                    url: u,
                    dimensions: i.into(),
                }),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Titles {
    pub en: Option<String>,
    pub en_jp: String,
    pub ja_jp: String,
}

impl From<kitsu::Titles> for Titles {
    fn from(value: kitsu::Titles) -> Self {
        Self {
            en: value.en,
            en_jp: value.en_jp,
            ja_jp: value.ja_jp,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Show {
    pub id: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub slug: String,
    pub synopsis: String,
    pub description: String,
    pub canonical_title: String,
    pub start_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    pub poster_image: Images,
    pub cover_image: Option<Images>,
    pub episode_count: Option<u32>,
    pub episode_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_video_id: Option<String>,
    pub nsfw: bool,
}

impl TryFrom<kitsu::Anime> for Show {
    type Error = ParseIntError;

    fn try_from(value: kitsu::Anime) -> Result<Self, Self::Error> {
        let id = value.id.parse()?;

        Ok(Self {
            id,
            created_at: value.attributes.created_at,
            updated_at: value.attributes.updated_at,
            slug: value.attributes.slug,
            synopsis: value.attributes.synopsis,
            description: value.attributes.description,
            canonical_title: value.attributes.canonical_title,
            start_date: value.attributes.start_date,
            end_date: value.attributes.end_date,
            poster_image: value.attributes.poster_image.into(),
            cover_image: value.attributes.cover_image.map(Into::into),
            episode_count: value.attributes.episode_count,
            episode_length: value.attributes.episode_length,
            total_length: value.attributes.total_length,
            youtube_video_id: value.attributes.youtube_video_id,
            nsfw: value.attributes.nsfw,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadGroup {
    pub title: String,
    #[serde(flatten)]
    pub variant: DownloadVariant,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub downloads: Vec<Download>,
}

impl From<nyaa::AnimeDownloads> for DownloadGroup {
    fn from(value: nyaa::AnimeDownloads) -> Self {
        let created_at = value
            .downloads
            .iter()
            .map(|d| d.pub_date)
            .min()
            .unwrap_or_default();
        let updated_at = value
            .downloads
            .iter()
            .map(|d| d.pub_date)
            .max()
            .unwrap_or_default();
        Self {
            title: value.title,
            variant: value.variant.into(),
            created_at,
            updated_at,
            downloads: value.downloads.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<DownloadGroup> for proto::api::v2::DownloadCollection {
    fn from(value: DownloadGroup) -> Self {
        proto::api::v2::DownloadCollection {
            created_at: Some(prost_timestamp(value.created_at)),
            updated_at: Some(prost_timestamp(value.updated_at)),
            title: value.title,
            variant: Some(value.variant.into()),
            downloads: value.downloads.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum DownloadVariant {
    Batch(RangeInclusive<u32>),
    Episode(Episode),
    Movie,
}

impl From<nyaa::DownloadVariant> for DownloadVariant {
    fn from(value: nyaa::DownloadVariant) -> Self {
        match value {
            nyaa::DownloadVariant::Batch(range) => DownloadVariant::Batch(range),
            nyaa::DownloadVariant::Episode(ep) => DownloadVariant::Episode(ep.into()),
            nyaa::DownloadVariant::Movie => DownloadVariant::Movie,
        }
    }
}

impl From<DownloadVariant> for proto::api::v2::download_collection::Variant {
    fn from(value: DownloadVariant) -> Self {
        match value {
            DownloadVariant::Batch(range) => {
                proto::api::v2::download_collection::Variant::Batch(proto::api::v2::Batch {
                    start: *range.start(),
                    end: *range.end(),
                })
            }
            DownloadVariant::Episode(ep) => {
                proto::api::v2::download_collection::Variant::Episode(ep.into())
            }
            DownloadVariant::Movie => {
                proto::api::v2::download_collection::Variant::Movie(proto::api::v2::Movie {})
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Episode {
    pub episode: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimal: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

impl From<nyaa::Episode> for Episode {
    fn from(value: nyaa::Episode) -> Self {
        Self {
            episode: value.episode,
            decimal: value.decimal,
            version: value.version,
            extra: value.extra,
        }
    }
}

impl From<Episode> for proto::api::v2::Episode {
    fn from(value: Episode) -> Self {
        Self {
            number: value.episode,
            decimal: value.decimal.unwrap_or_default(),
            version: value.version.unwrap_or_default(),
            extra: value.extra.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Download {
    pub comments: String,
    pub resolution: u16,
    pub torrent: String,
    pub file_name: String,
    pub published_date: DateTime<Utc>,
}

impl From<nyaa::Download> for Download {
    fn from(value: nyaa::Download) -> Self {
        Self {
            comments: value.comments,
            resolution: value.resolution,
            torrent: value.torrent,
            file_name: value.file_name,
            published_date: value.pub_date,
        }
    }
}

impl From<Download> for proto::api::v2::Download {
    fn from(value: Download) -> Self {
        Self {
            published_date: Some(prost_timestamp(value.published_date)),
            resolution: u32::from(value.resolution),
            comments: value.comments,
            torrent: value.torrent,
            file_name: value.file_name,
        }
    }
}

fn prost_timestamp(date_time: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: date_time.timestamp(),
        nanos: date_time.timestamp_subsec_nanos().cast_signed(),
    }
}
