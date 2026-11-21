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

pub(crate) mod shows {
    use axum::Json;
    use axum::extract::{Path, Query, State};
    use serde::Deserialize;
    use tmdb::TmdbClient;
    use tmdb::search::TvSearchParams;
    use tracing::warn;

    use crate::errors::Error;
    use crate::models::{Page, Show, ShowSearchResult};

    #[derive(Debug, Deserialize)]
    pub(crate) struct SearchQuery {
        query: String,
        page: Option<u32>,
        language: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub(crate) struct LanguageQuery {
        language: Option<String>,
    }

    pub(crate) async fn search(
        Query(params): Query<SearchQuery>,
        State(tmdb): State<TmdbClient>,
    ) -> Result<Json<Page<ShowSearchResult>>, Error> {
        let params = TvSearchParams {
            query: params.query,
            page: params.page,
            language: params.language,
            ..TvSearchParams::default()
        };
        let page = tmdb::search::tv(&tmdb, &params).await?;
        Ok(Json(page.into()))
    }

    pub(crate) async fn by_id(
        Path(id): Path<u32>,
        Query(params): Query<LanguageQuery>,
        State(tmdb): State<TmdbClient>,
    ) -> Result<Json<Show>, Error> {
        let language = params.language.as_deref();
        let (details, external_ids) = tokio::join!(
            tmdb::tv::details(&tmdb, id, language),
            tmdb::tv::external_ids(&tmdb, id)
        );
        let details = details?;
        // The TVDB id is a nice-to-have, so a failure here must not fail the request.
        let external_ids = external_ids
            .inspect_err(|error| warn!(%error, id, "failed to fetch external ids"))
            .ok();
        Ok(Json(Show::new(details, external_ids)))
    }
}

pub(crate) mod batch {
    use std::convert::Infallible;

    use async_stream::try_stream;
    use axum::Json;
    use axum::extract::{Query, State};
    use axum::response::Sse;
    use axum::response::sse::{Event, KeepAlive};
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
    use axum::Json;
    use axum::extract::{Query, State};
    use axum::response::Sse;
    use axum::response::sse::{Event, KeepAlive};
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
    use axum::Json;
    use axum::extract::{Query, State};
    use axum::response::Sse;
    use axum::response::sse::{Event, KeepAlive};
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
    use axum::Json;
    use axum::extract::{Query, State};
    use axum::response::Sse;
    use axum::response::sse::{Event, KeepAlive};
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
                        Ok(event) => yield event,
                        Err(e) => error!(error = ?e, "failed to serialize"),
                    }
                    Err(e) => error!(error = ?e, "sender closed"),
                }
            }
        };
        Sse::new(stream).keep_alive(KeepAlive::new())
    }
}
