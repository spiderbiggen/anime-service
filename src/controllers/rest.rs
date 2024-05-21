use serde::Deserialize;

use crate::datasource::repository;
use crate::datasource::repository::downloads::{QueryOptions, Variant};
use crate::errors::Error;
use crate::models::DownloadGroup;
use crate::state::DBPool;

#[derive(Debug, Deserialize)]
pub(crate) struct DownloadQuery {
    title: Option<String>,
}

async fn find_downloads(
    params: DownloadQuery,
    pool: DBPool,
    variant: Option<Variant>,
) -> Result<Vec<DownloadGroup>, Error> {
    let options = QueryOptions {
        title: params.title,
    };
    let downloads = repository::downloads::get_with_downloads(pool, variant, Some(options)).await?;
    Ok(downloads)
}

pub(crate) mod anime {
    use crate::errors::Error;
    use crate::models;
    use crate::state::ReqwestClient;
    use axum::extract::{Path, State};
    use axum::Json;

    pub(crate) async fn by_id(
        Path(id): Path<u32>,
        State(reqwest): State<ReqwestClient>,
    ) -> Result<Json<models::Show>, Error> {
        let anime = kitsu::anime::single(&reqwest, id).await?;
        let show = anime.data.try_into()?;
        Ok(Json(show))
    }

    pub(crate) async fn find(
        State(reqwest): State<ReqwestClient>,
    ) -> Result<Json<Vec<models::Show>>, Error> {
        let anime = kitsu::anime::collection(&reqwest).await?;
        let show: Result<Vec<_>, _> = anime.data.into_iter().map(|d| d.try_into()).collect();
        Ok(Json(show?))
    }
}

pub(crate) mod batch {
    use std::convert::Infallible;

    use async_stream::try_stream;
    use axum::extract::{Query, State};
    use axum::response::sse::{Event, KeepAlive};
    use axum::response::Sse;
    use axum::Json;
    use futures::Stream;
    use tracing::error;

    use crate::controllers::rest::DownloadQuery;
    use crate::datasource::repository::downloads::Variant;
    use crate::errors::Error;
    use crate::models::{DownloadGroup, DownloadVariant};
    use crate::state::{AppState, DBPool};

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<Vec<DownloadGroup>>, Error> {
        let downloads = super::find_downloads(params, pool, Some(Variant::Batch)).await?;
        Ok(Json(downloads))
    }

    pub(crate) async fn get_downloads_events(
        State(state): State<AppState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let mut rx = state.downloads_channel.subscribe();
        let stream = try_stream! {
            loop {
                match rx.recv().await {
                    Ok(i) => if let DownloadVariant::Batch(_) = i.variant {
                        match Event::default().event("download").json_data(i) {
                            Ok(event) => yield  event,
                            Err(e) => error!(error = ?e, "failed to serialize"),
                        }
                    }
                    Err(e) => error!(error = ?e, "sender closed"),
                }
            }
        };
        Sse::new(stream).keep_alive(KeepAlive::new())
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
    use crate::datasource::repository::downloads::Variant;
    use crate::errors::Error;
    use crate::models::{DownloadGroup, DownloadVariant};
    use crate::state::{AppState, DBPool};

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<Vec<DownloadGroup>>, Error> {
        let downloads = super::find_downloads(params, pool, Some(Variant::Episode)).await?;
        Ok(Json(downloads))
    }

    pub(crate) async fn get_downloads_events(
        State(state): State<AppState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let mut rx = state.downloads_channel.subscribe();
        let stream = try_stream! {
            loop {
                match rx.recv().await {
                    Ok(i) => if let DownloadVariant::Episode(_) = i.variant { match Event::default().event("download").json_data(i) {
                        Ok(event) => yield  event,
                        Err(e) => error!(error = ?e, "failed to serialize"),
                    } }
                    Err(e) => error!(error = ?e, "sender closed"),
                }
            }
        };
        Sse::new(stream).keep_alive(KeepAlive::new())
    }
}

pub(crate) mod movie {
    use std::convert::Infallible;

    use async_stream::try_stream;
    use axum::extract::{Query, State};
    use axum::response::sse::{Event, KeepAlive};
    use axum::response::Sse;
    use axum::Json;
    use futures::Stream;
    use tracing::error;

    use crate::controllers::rest::DownloadQuery;
    use crate::datasource::repository::downloads::Variant;
    use crate::errors::Error;
    use crate::models::{DownloadGroup, DownloadVariant};
    use crate::state::{AppState, DBPool};

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<Vec<DownloadGroup>>, Error> {
        let downloads = super::find_downloads(params, pool, Some(Variant::Movie)).await?;
        Ok(Json(downloads))
    }

    pub(crate) async fn get_downloads_events(
        State(state): State<AppState>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let mut rx = state.downloads_channel.subscribe();
        let stream = try_stream! {
            loop {
                match rx.recv().await {
                    Ok(i) => if let DownloadVariant::Movie = i.variant { match Event::default().event("download").json_data(i) {
                        Ok(event) => yield  event,
                        Err(e) => error!(error = ?e, "failed to serialize"),
                    } }
                    Err(e) => error!(error = ?e, "sender closed"),
                }
            }
        };
        Sse::new(stream).keep_alive(KeepAlive::new())
    }
}

pub mod downloads {
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
    use crate::datasource::repository::downloads::QueryOptions;
    use crate::errors::Error;
    use crate::models::DownloadGroup;
    use crate::state::{AppState, DBPool};

    pub(crate) async fn find_downloads(
        Query(params): Query<DownloadQuery>,
        State(pool): State<DBPool>,
    ) -> Result<Json<Vec<DownloadGroup>>, Error> {
        let options = QueryOptions {
            title: params.title,
        };
        let downloads =
            repository::downloads::get_with_downloads(pool.clone(), None, Some(options)).await?;
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
                    }
                    Err(e) => error!(error = ?e, "sender closed"),
                }
            }
        };
        Sse::new(stream).keep_alive(KeepAlive::new())
    }
}
