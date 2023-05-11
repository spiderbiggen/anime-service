use std::convert::Infallible;
use std::result::Result;

use async_stream::try_stream;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive};
use axum::response::Sse;
use axum::{extract::Query, Json};
use chrono::Duration;
use futures::Stream;
use serde::Deserialize;
use tracing::error;

use repository::episode::EpisodeQueryOptions;

use crate::datasource::repository;
use crate::errors::Error;
use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;
use crate::state::{AppState, DBPool};

#[derive(Debug, Deserialize)]
pub(crate) struct DownloadQuery {
    title: Option<String>,
}

pub(crate) async fn get(
    Query(params): Query<DownloadQuery>,
    State(pool): State<DBPool>,
    State(cache): State<RequestCache<Vec<DownloadGroup>>>,
) -> Result<Json<Vec<DownloadGroup>>, Error> {
    let key = params.title.clone().unwrap_or_default();
    if let Some(cache) = cache.get(&key) {
        return Ok(Json(cache.to_vec()));
    }
    let options = EpisodeQueryOptions {
        title: params.title,
    };
    let downloads = repository::episode::get_with_downloads(pool, Some(options)).await?;
    let json = Json(downloads.clone());
    if key.is_empty() {
        cache.insert_with_timeout(&key, downloads, Duration::hours(1));
    } else {
        cache.insert_with_default_timeout(&key, downloads);
    }
    Ok(json)
}

pub(crate) async fn get_updates(
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
