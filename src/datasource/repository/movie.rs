use std::cmp::Reverse;

use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, FromRow, Postgres, QueryBuilder};

use crate::datasource::repository::{download, RawSingleResult, SingleResult};
use crate::models::{DownloadGroup, DownloadVariant};
use crate::state::DBPool;

#[derive(Debug, FromRow)]
struct MovieEntity {
    id: Uuid,
    #[sqlx(rename = "provider")]
    _provider: String,
    title: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct MovieQueryOptions {
    pub title: Option<String>,
}

pub(super) async fn insert<'e, E>(
    executor: E,
    title: &str,
    created_at: &DateTime<Utc>,
    updated_at: &DateTime<Utc>,
) -> anyhow::Result<Uuid>
where
    E: Executor<'e, Database = Postgres>,
{
    let record = sqlx::query_file!(
        "queries/movie/insert_movie_download.sql",
        "SubsPlease",
        title,
        created_at,
        updated_at,
    )
    .fetch_one(executor)
    .await?;
    Ok(record.id)
}

pub(super) async fn get_by_unique_index<'e, E>(
    executor: E,
    title: &str,
) -> anyhow::Result<Option<SingleResult>>
where
    E: Executor<'e, Database = Postgres>,
{
    let record = sqlx::query_file_as!(
        RawSingleResult,
        "queries/movie/query_movie_download_by_unique.sql",
        "SubsPlease",
        title,
    )
    .fetch_optional(executor)
    .await?
    .map(|record| record.into());
    Ok(record)
}

pub(super) async fn update<'e, E>(
    executor: E,
    id: Uuid,
    updated_at: &DateTime<Utc>,
) -> anyhow::Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_file!(
        "queries/movie/update_movie_download_updated_at.sql",
        &id,
        updated_at
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub(super) async fn upsert<C>(
    conn: &mut C,
    title: &str,
    created_at: &DateTime<Utc>,
    updated_at: &DateTime<Utc>,
) -> anyhow::Result<(Uuid, Vec<u16>)>
where
    C: Connection<Database = Postgres>,
{
    let mut transaction = conn.begin().await?;
    if let Some(record) = get_by_unique_index(&mut *transaction, title).await? {
        if record.updated_at < *updated_at {
            update(&mut *transaction, record.id, updated_at).await?;
        }
        transaction.commit().await?;
        return Ok((record.id, record.resolutions));
    }
    let id = insert(&mut *transaction, title, created_at, updated_at).await?;
    transaction.commit().await?;
    Ok((id, Vec::new()))
}

pub async fn get_with_downloads(
    conn: DBPool,
    options: Option<MovieQueryOptions>,
) -> anyhow::Result<Vec<DownloadGroup>> {
    let mut transaction = conn.begin().await?;
    let rows = get_data_movies(&mut *transaction, options).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let movie_ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let mut downloads = download::get_for_movies(&mut *transaction, &movie_ids).await?;
    transaction.commit().await?;

    let result: anyhow::Result<Vec<_>> = rows
        .into_iter()
        .map(|r| {
            Ok(DownloadGroup {
                title: r.title,
                variant: DownloadVariant::Movie,
                created_at: r.created_at,
                updated_at: r.updated_at,
                downloads: downloads.remove(&r.id).unwrap_or_default(),
            })
        })
        .collect();
    let mut movies = result?;
    movies.sort_by_key(|ep| Reverse(ep.updated_at));
    Ok(movies)
}

async fn get_data_movies<'e, E>(
    executor: E,
    options: Option<MovieQueryOptions>,
) -> anyhow::Result<Vec<MovieEntity>>
where
    E: Executor<'e, Database = Postgres>,
{
    let mut qb = QueryBuilder::new("SELECT * FROM movie_download");
    if let Some(options) = options {
        if let Some(title) = options.title {
            qb.push(" WHERE title ILIKE ").push_bind(title);
        }
    }
    let query = qb
        .push(" ORDER BY updated_at DESC")
        .push(" LIMIT 25")
        .build_query_as::<MovieEntity>();

    let rows = query.fetch_all(executor).await?;
    Ok(rows)
}
