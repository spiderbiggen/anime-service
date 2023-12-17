use serde::Deserialize;

pub(crate) mod unversioned {
    use std::convert::Infallible;

    use async_stream::try_stream;
    use axum::extract::{Path, Query, State};
    use axum::response::sse::{Event, KeepAlive};
    use axum::response::Sse;
    use axum::Json;
    use futures::Stream;
    use serde_json::json;
    use sqlx::types::JsonValue;
    use tracing::error;

    use crate::controllers::rest::DownloadQuery;
    use crate::datasource::repository;
    use crate::datasource::repository::episode::EpisodeQueryOptions;
    use crate::errors::Error;
    use crate::models;
    use crate::models::{Download, DownloadGroup, DownloadVariant};
    use crate::state::{AppState, DBPool, ReqwestClient};

    pub(crate) async fn anime_by_id(
        Path(id): Path<u32>,
        State(hyper): State<ReqwestClient>,
    ) -> Result<Json<models::Show>, Error> {
        let anime = kitsu::anime::single(hyper, id).await?;
        let show = anime.data.try_into()?;
        Ok(Json(show))
    }

    pub(crate) async fn find_anime(
        State(hyper): State<ReqwestClient>,
    ) -> Result<Json<Vec<models::Show>>, Error> {
        let anime = kitsu::anime::collection(hyper).await?;
        let show: Result<Vec<_>, _> = anime.data.into_iter().map(|d| d.try_into()).collect();
        Ok(Json(show?))
    }

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<JsonValue>, Error> {
        let options = EpisodeQueryOptions {
            title: params.title,
        };
        let downloads =
            repository::episode::get_with_downloads(pool.clone(), Some(options)).await?;
        let json = downloads
            .into_iter()
            .filter_map(map_old_download_group)
            .collect();
        Ok(Json(json))
    }

    pub(crate) async fn get_downloads_events(
        State(state): State<AppState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let mut rx = state.downloads_channel.subscribe();
        let stream = try_stream! {
            loop {
                match rx.recv().await {
                    Ok(i) => {
                            if let Some(group) = map_old_download_group(i) {
                                match Event::default().event("download").json_data(group) {
                                    Ok(event) => yield  event,
                                    Err(e) => error!(error = ?e, "failed to serialize"),
                                }
                            }
                        }
                    Err(e) => error!(error = ?e, "sender closed"),
                }
            };
        };
        Sse::new(stream).keep_alive(KeepAlive::new())
    }

    fn map_old_download_group(group: DownloadGroup) -> Option<JsonValue> {
        let DownloadVariant::Episode(episode) = group.variant else {
            return None;
        };

        let mut json = json!({
            "title": group.title,
            "episode": episode.episode,
            "created_at": group.created_at,
            "updated_at": group.updated_at,
            "downloads": group.downloads.into_iter().map(map_download).collect::<Vec<_>>(),
        });
        if let JsonValue::Object(map) = &mut json {
            if let Some(decimal) = episode.decimal {
                map.insert(String::from("decimal"), json!(decimal));
            }
            if let Some(version) = episode.version {
                map.insert(String::from("version"), json!(version));
            }
            if let Some(extra) = episode.extra {
                map.insert(String::from("extra"), json!(extra));
            }
        }
        Some(json)
    }

    fn map_download(download: Download) -> JsonValue {
        json!({
            "comments": download.comments,
            "resolution": format_args!("{}p", download.resolution),
            "torrent": download.torrent,
            "file_name": download.file_name,
            "published_date": download.published_date,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct DownloadQuery {
    title: Option<String>,
}

pub(crate) mod batch {
    use axum::extract::{Query, State};
    use axum::Json;

    use crate::controllers::rest::DownloadQuery;
    use crate::datasource::repository;
    use crate::datasource::repository::batch::BatchQueryOptions;
    use crate::errors::Error;
    use crate::models::DownloadGroup;
    use crate::state::DBPool;

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<Vec<DownloadGroup>>, Error> {
        let options = BatchQueryOptions {
            title: params.title,
        };
        let downloads = repository::batch::get_with_downloads(pool.clone(), Some(options)).await?;
        Ok(Json(downloads))
    }
}

pub(crate) mod episode {
    use std::convert::Infallible;

    use async_stream::try_stream;
    use axum::extract::{Query, State};
    use axum::response::sse::{Event, KeepAlive};
    use axum::response::Sse;
    use axum::Json;
    use futures::Stream;
    use tracing::error;

    use crate::controllers::rest::DownloadQuery;
    use crate::datasource::repository;
    use crate::datasource::repository::episode::EpisodeQueryOptions;
    use crate::errors::Error;
    use crate::models::DownloadGroup;
    use crate::state::{AppState, DBPool};

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<Vec<DownloadGroup>>, Error> {
        let options = EpisodeQueryOptions {
            title: params.title,
        };
        let downloads =
            repository::episode::get_with_downloads(pool.clone(), Some(options)).await?;
        Ok(Json(downloads))
    }

    pub(crate) async fn get_downloads_events(
        State(state): State<AppState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let mut rx = state.downloads_channel.subscribe();
        let stream = try_stream! {
            loop {
                match rx.recv().await {
                    Ok(i) => match Event::default().event("download").json_data(i) {
                        Ok(event) => yield  event,
                        Err(e) => error!(error = ?e, "failed to serialize"),
                    },
                    Err(e) => error!(error = ?e, "sender closed"),
                }
            }
        };
        Sse::new(stream).keep_alive(KeepAlive::new())
    }
}

pub(crate) mod movie {
    use axum::extract::{Query, State};
    use axum::Json;

    use crate::controllers::rest::DownloadQuery;
    use crate::datasource::repository;
    use crate::datasource::repository::movie::MovieQueryOptions;
    use crate::errors::Error;
    use crate::models::DownloadGroup;
    use crate::state::DBPool;

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<Vec<DownloadGroup>>, Error> {
        let options = MovieQueryOptions {
            title: params.title,
        };
        let downloads = repository::movie::get_with_downloads(pool.clone(), Some(options)).await?;
        Ok(Json(downloads))
    }
}
