use std::result::Result;

use axum::extract::State;
use axum::{extract::Query, Json};
use serde::Deserialize;
use sqlx::{Pool, Postgres};

use crate::datasource::repositories;
use crate::errors::Error;
use crate::models;

#[derive(Debug, Deserialize)]
pub(crate) struct DownloadQuery {
    title: Option<String>,
}

pub(crate) async fn get(
    Query(params): Query<DownloadQuery>,
    State(pool): State<Pool<Postgres>>,
) -> Result<Json<Vec<models::DownloadGroup>>, Error> {
    let options = repositories::downloads::EpisodeQueryOptions {
        title: params.title,
    };
    let downloads =
        repositories::downloads::get_episode_with_downloads(&pool, Some(options)).await?;
    Ok(Json(downloads))
}
