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
    fn from(image_dimension: kitsu::ImageDimension) -> Self {
        Self {
            width: image_dimension.width,
            height: image_dimension.height,
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
    fn from(images: kitsu::Images) -> Self {
        Self {
            original: images.original,
            large: images
                .large
                .zip(images.meta.dimensions.large)
                .map(|(u, i)| ImageDefinition {
                    url: u,
                    dimensions: i.into(),
                }),
            medium: images
                .medium
                .zip(images.meta.dimensions.medium)
                .map(|(u, i)| ImageDefinition {
                    url: u,
                    dimensions: i.into(),
                }),
            small: images
                .small
                .zip(images.meta.dimensions.small)
                .map(|(u, i)| ImageDefinition {
                    url: u,
                    dimensions: i.into(),
                }),
            tiny: images
                .tiny
                .zip(images.meta.dimensions.tiny)
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
    fn from(titles: kitsu::Titles) -> Self {
        Self {
            en: titles.en,
            en_jp: titles.en_jp,
            ja_jp: titles.ja_jp,
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
    pub end_date: String,
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
            cover_image: value.attributes.cover_image.map(|c| c.into()),
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
    fn from(a: nyaa::AnimeDownloads) -> Self {
        let created_at = a
            .downloads
            .iter()
            .map(|d| d.pub_date)
            .min()
            .unwrap_or_default();
        let updated_at = a
            .downloads
            .iter()
            .map(|d| d.pub_date)
            .max()
            .unwrap_or_default();
        Self {
            title: a.title,
            variant: a.variant.into(),
            created_at,
            updated_at,
            downloads: a.downloads.into_iter().map(|it| it.into()).collect(),
        }
    }
}

impl From<DownloadGroup> for Option<proto::api::v1::DownloadCollection> {
    fn from(val: DownloadGroup) -> Self {
        let DownloadVariant::Episode(episode) = val.variant else {
            return None;
        };
        Some(proto::api::v1::DownloadCollection {
            episode: Some(proto::api::v1::Episode {
                created_at: Some(prost_timestamp(val.created_at)),
                updated_at: Some(prost_timestamp(val.updated_at)),
                title: val.title,
                number: episode.episode,
                decimal: episode.decimal.unwrap_or_default(),
                version: episode.version.unwrap_or_default(),
                extra: episode.extra.unwrap_or_default(),
            }),
            downloads: val.downloads.into_iter().map(|d| d.into()).collect(),
        })
    }
}

impl From<DownloadGroup> for Option<proto::api::v2::DownloadCollection> {
    fn from(val: DownloadGroup) -> Self {
        Some(proto::api::v2::DownloadCollection {
            created_at: Some(prost_timestamp(val.created_at)),
            updated_at: Some(prost_timestamp(val.updated_at)),
            title: val.title,
            variant: Some(val.variant.into()),
            downloads: val.downloads.into_iter().map(|d| d.into()).collect(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
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
    fn from(a: nyaa::Episode) -> Self {
        Self {
            episode: a.episode,
            decimal: a.decimal,
            version: a.version,
            extra: a.extra,
        }
    }
}

impl From<Episode> for proto::api::v2::Episode {
    fn from(val: Episode) -> Self {
        Self {
            number: val.episode,
            decimal: val.decimal.unwrap_or_default(),
            version: val.version.unwrap_or_default(),
            extra: val.extra.unwrap_or_default(),
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
    fn from(a: nyaa::Download) -> Self {
        Self {
            comments: a.comments,
            resolution: a.resolution,
            torrent: a.torrent,
            file_name: a.file_name,
            published_date: a.pub_date,
        }
    }
}

impl From<Download> for proto::api::v1::Download {
    fn from(val: Download) -> Self {
        Self {
            published_date: Some(prost_timestamp(val.published_date)),
            resolution: format!("{}p", val.resolution),
            comments: val.comments,
            torrent: val.torrent,
            file_name: val.file_name,
        }
    }
}

impl From<Download> for proto::api::v2::Download {
    fn from(val: Download) -> Self {
        Self {
            published_date: Some(prost_timestamp(val.published_date)),
            resolution: val.resolution as u32,
            comments: val.comments,
            torrent: val.torrent,
            file_name: val.file_name,
        }
    }
}

fn prost_timestamp(date_time: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: date_time.timestamp(),
        nanos: date_time.timestamp_subsec_nanos() as i32,
    }
}
