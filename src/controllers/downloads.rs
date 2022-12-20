use crate::errors::{Error, InternalError};
use crate::sql_models::EpisodeWithResolutions;
use crate::{models, sql_models};
use axum::extract::State;
use axum::{extract::Query, Json};
use serde::Deserialize;
use sqlx::query_builder::QueryBuilder;
use sqlx::types::Uuid;
use sqlx::{Pool, Postgres};
use std::cmp::Reverse;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::result::Result;
use tokio_stream::StreamExt;

#[derive(Debug, Deserialize)]
pub(crate) struct DownloadQuery {
    title: Option<String>,
}

pub(crate) async fn get(
    Query(params): Query<DownloadQuery>,
    State(pool): State<Pool<Postgres>>,
) -> Result<Json<Vec<models::DownloadGroup>>, Error> {
    Ok(Json(get_groups(pool, params.title.as_deref()).await?))
}

async fn get_groups(
    pool: Pool<Postgres>,
    title: Option<&str>,
) -> Result<Vec<models::DownloadGroup>, InternalError> {
    let mut qb = QueryBuilder::new("SELECT * FROM episode_download");
    let mut has_where = false;
    if let Some(title) = title {
        qb.push(if has_where { " AND" } else { " WHERE" })
            .push(" title ILIKE ")
            .push_bind(title);
    }
    let query = qb
        .push(" ORDER BY created_at DESC")
        .push(" LIMIT 25")
        .build_query_as::<sql_models::Episode>();
    let rows = query.fetch_all(&pool).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let iter = rows.into_iter().map(|r| {
        let id = r.id.clone();
        let group = EpisodeWithResolutions {
            episode: r,
            resolutions: vec![],
        };
        (id, group)
    });
    let mut map: HashMap<Uuid, EpisodeWithResolutions, RandomState> = HashMap::from_iter(iter);

    let mut qb = QueryBuilder::new("SELECT * FROM episode_download_resolution");
    qb.push(" WHERE episode_download_id in (");
    let mut separated = qb.separated(", ");
    for &id in map.keys() {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    qb.push(" ORDER BY array_position(array['2160p', '1080p', '720p', '480p'], resolution)");
    let query = qb.build_query_as::<sql_models::Download>();
    let mut stream = query.fetch(&pool);
    while let Some(row) = stream.next().await {
        let row = row?;
        if let Some(group) = map.get_mut(&row.episode_download_id) {
            group.resolutions.push(row);
        }
    }
    let mut episodes = map
        .into_iter()
        .map(|(_, v)| v.try_into())
        .collect::<Result<Vec<models::DownloadGroup>, _>>()?;
    episodes.sort_by_key(|ep| Reverse(ep.episode.published_date));
    Ok(episodes)
}
