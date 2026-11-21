use std::ops::RangeInclusive;

use jiff::Timestamp;
use jiff::civil::Date;
use serde::Serialize;
use tmdb::images::{BackdropSize, PosterSize, backdrop_url, poster_url};
use tmdb::search::TvSearchResult;
use tmdb::tv::{ExternalIds, TvDetails};

/// A page of results.
#[derive(Serialize, Clone, Debug)]
pub struct Page<T> {
    pub page: u32,
    pub total_pages: u32,
    pub total_results: u32,
    pub results: Vec<T>,
}

impl<T, U> From<tmdb::ListResponse<U>> for Page<T>
where
    T: From<U>,
{
    fn from(value: tmdb::ListResponse<U>) -> Self {
        Self {
            page: value.page,
            total_pages: value.total_pages,
            total_results: value.total_results,
            results: value.results.into_iter().map(Into::into).collect(),
        }
    }
}

fn poster(path: Option<String>) -> Option<String> {
    path.map(|path| poster_url(&path, PosterSize::W500))
}

fn backdrop(path: Option<String>) -> Option<String> {
    path.map(|path| backdrop_url(&path, BackdropSize::W780))
}

/// A show in search results.
#[derive(Serialize, Clone, Debug)]
pub struct ShowSearchResult {
    pub id: u32,
    pub name: String,
    pub original_name: String,
    pub overview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_air_date: Option<Date>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_url: Option<String>,
    pub origin_country: Vec<String>,
    pub genre_ids: Vec<u32>,
    pub adult: bool,
}

impl From<TvSearchResult> for ShowSearchResult {
    fn from(value: TvSearchResult) -> Self {
        Self {
            id: value.id,
            name: value.name,
            original_name: value.original_name,
            overview: value.overview,
            first_air_date: value.first_air_date,
            poster_url: poster(value.poster_path),
            backdrop_url: backdrop(value.backdrop_path),
            origin_country: value.origin_country,
            genre_ids: value.genre_ids,
            adult: value.adult,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Genre {
    pub id: u32,
    pub name: String,
}

/// The details of a show.
#[derive(Serialize, Clone, Debug)]
pub struct Show {
    pub id: u32,
    /// The `TheTVDB` id, which Sonarr uses. Missing when TMDB does not know it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb_id: Option<u32>,
    pub name: String,
    pub original_name: String,
    pub overview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_air_date: Option<Date>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_air_date: Option<Date>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_episodes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_seasons: Option<u32>,
    pub adult: bool,
    pub genres: Vec<Genre>,
    pub origin_country: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_url: Option<String>,
}

impl Show {
    #[must_use]
    pub fn new(details: TvDetails, external_ids: Option<ExternalIds>) -> Self {
        Self {
            id: details.id,
            tvdb_id: external_ids.and_then(|ids| ids.tvdb_id),
            name: details.name,
            original_name: details.original_name,
            overview: details.overview,
            status: details.status,
            first_air_date: details.first_air_date,
            last_air_date: details.last_air_date,
            number_of_episodes: details.number_of_episodes,
            number_of_seasons: details.number_of_seasons,
            adult: details.adult,
            genres: details
                .genres
                .into_iter()
                .map(|genre| Genre {
                    id: genre.id,
                    name: genre.name,
                })
                .collect(),
            origin_country: details.origin_country,
            poster_url: poster(details.poster_path),
            backdrop_url: backdrop(details.backdrop_path),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadGroup {
    pub title: String,
    #[serde(flatten)]
    pub variant: DownloadVariant,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
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

#[derive(Debug, Clone, Serialize)]
pub struct Download {
    pub comments: String,
    pub resolution: u16,
    pub torrent: String,
    pub file_name: String,
    pub published_date: Timestamp,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn details() -> TvDetails {
        serde_json::from_str(
            r#"{
                "id": 7, "name": "Sample", "original_name": "サンプル", "overview": "o",
                "status": "Ended", "first_air_date": "2023-09-29", "last_air_date": "",
                "number_of_episodes": 12, "number_of_seasons": 1,
                "genres": [{ "id": 16, "name": "Animation" }],
                "origin_country": ["JP"],
                "poster_path": "/p.jpg", "backdrop_path": null
            }"#,
        )
        .expect("valid details")
    }

    fn external_ids(tvdb_id: Option<u32>) -> ExternalIds {
        ExternalIds {
            id: 7,
            imdb_id: None,
            tvdb_id,
        }
    }

    #[test]
    fn show_uses_the_tvdb_id_and_builds_image_urls() {
        let show = Show::new(details(), Some(external_ids(Some(99))));

        assert_eq!(show.tvdb_id, Some(99));
        assert_eq!(
            show.poster_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w500/p.jpg")
        );
        assert_eq!(show.backdrop_url, None);
        assert_eq!(show.last_air_date, None);
        assert_eq!(show.genres[0].name, "Animation");
    }

    #[test]
    fn show_without_external_ids_has_no_tvdb_id() {
        assert_eq!(Show::new(details(), None).tvdb_id, None);
        assert_eq!(Show::new(details(), Some(external_ids(None))).tvdb_id, None);
    }

    #[test]
    fn show_skips_missing_values_when_serialized() {
        let json = serde_json::to_value(Show::new(details(), None)).expect("serializes");

        assert!(json.get("tvdb_id").is_none());
        assert!(json.get("backdrop_url").is_none());
        assert_eq!(json["first_air_date"], "2023-09-29");
    }

    #[test]
    fn page_maps_every_result() {
        let page: tmdb::ListResponse<TvSearchResult> = serde_json::from_str(
            r#"{ "page": 2, "total_pages": 5, "total_results": 90,
                 "results": [{ "id": 1, "name": "A", "backdrop_path": "/b.jpg" }] }"#,
        )
        .expect("valid page");

        let page: Page<ShowSearchResult> = page.into();

        assert_eq!(
            (page.page, page.total_pages, page.total_results),
            (2, 5, 90)
        );
        assert_eq!(page.results[0].name, "A");
        assert_eq!(
            page.results[0].backdrop_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w780/b.jpg")
        );
    }
}
