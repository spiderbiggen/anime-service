use crate::errors::{Error, InternalError};
use crate::{models, sql_models};
use axum::extract::State;
use axum::{extract::Query, Json};
use serde::Deserialize;
use sqlx::query_builder::QueryBuilder;
use sqlx::{Pool, Postgres};
use std::result::Result;

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
        qb.push(if has_where { " AND " } else { " WHERE " })
            .push("title ILIKE ")
            .push_bind(title);
    }
    let query = qb
        .push(" ORDER BY created_at DESC ")
        .push(" LIMIT 25 ")
        .build_query_as::<sql_models::Episode>();
    let rows = query.fetch_all(&pool).await?;
    let mut result = Vec::<models::DownloadGroup>::with_capacity(rows.len());
    for row in rows {
        let resolutions = sqlx::query_as!(
            sql_models::Download,
            "SELECT * FROM episode_download_resolution WHERE episode_download_id = $1",
            row.id,
        )
        .fetch_all(&pool)
        .await?;
        result.push(models::DownloadGroup {
            episode: row.try_into()?,
            downloads: resolutions
                .into_iter()
                .map(|a| a.try_into())
                .collect::<Result<Vec<_>, InternalError>>()?,
        })
    }
    Ok(result)
}
