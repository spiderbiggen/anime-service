use crate::errors::InternalError;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use kitsu::models as kitsu;
use serde::Serialize;
use std::num::ParseIntError;
use url::Url;

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
    #[serde(flatten)]
    pub episode: Episode,
    pub downloads: Vec<Download>,
}

impl From<nyaa::AnimeDownloads> for DownloadGroup {
    fn from(a: nyaa::AnimeDownloads) -> Self {
        let mut ep: Episode = a.episode.into();
        ep.published_date = a
            .downloads
            .iter()
            .map(|d| d.pub_date)
            .min()
            .unwrap_or_default()
            .clone();
        Self {
            episode: ep,
            downloads: a.downloads.into_iter().map(|it| it.into()).collect(),
        }
    }
}

impl TryFrom<sql_models::EpisodeWithResolutions> for DownloadGroup {
    type Error = InternalError;

    fn try_from(a: sql_models::EpisodeWithResolutions) -> Result<Self, Self::Error> {
        Ok(Self {
            episode: a.episode.try_into()?,
            downloads: a
                .resolutions
                .into_iter()
                .map(|it| it.try_into())
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Episode {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimal: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    pub published_date: DateTime<Utc>,
}

impl From<nyaa::Episode> for Episode {
    fn from(a: nyaa::Episode) -> Self {
        Self {
            title: a.title,
            episode: a.episode,
            decimal: a.decimal,
            version: a.version,
            published_date: Default::default(),
        }
    }
}

impl TryFrom<sql_models::Episode> for Episode {
    type Error = InternalError;

    fn try_from(a: sql_models::Episode) -> Result<Self, Self::Error> {
        let ep = match a.episode {
            Some(i) => Some(i.try_into()?),
            None => None,
        };
        let dec = match a.decimal {
            Some(i) => Some(i.try_into()?),
            None => None,
        };
        let ver = match a.version {
            Some(i) => Some(i.try_into()?),
            None => None,
        };
        Ok(Self {
            title: a.title,
            episode: ep,
            decimal: dec,
            version: ver,
            published_date: a.created_at,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Download {
    pub comments: String,
    pub resolution: String,
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

impl TryFrom<sql_models::Download> for Download {
    type Error = InternalError;

    fn try_from(a: sql_models::Download) -> Result<Self, Self::Error> {
        Ok(Self {
            comments: a
                .comments
                .ok_or(anyhow!("required Download.comments was None"))?,
            resolution: a.resolution,
            torrent: a.torrent,
            file_name: a.file_name,
            published_date: a.created_at,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectDownload {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimal: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    pub comments: String,
    pub resolution: String,
    pub torrent: String,
    pub file_name: String,
    pub published_date: DateTime<Utc>,
}

impl From<nyaa::NyaaEntry> for DirectDownload {
    fn from(a: nyaa::NyaaEntry) -> Self {
        Self {
            title: a.title,
            episode: a.episode,
            decimal: a.decimal,
            version: a.version,
            comments: a.comments,
            resolution: a.resolution,
            torrent: a.torrent,
            file_name: a.file_name,
            published_date: a.pub_date,
        }
    }
}
