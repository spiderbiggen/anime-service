use std::cmp::Reverse;
use std::ops::RangeInclusive;

use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, FromRow, Postgres, QueryBuilder};

use crate::datasource::repository::{download, RawSingleResult, SingleResult};
use crate::models::{DownloadGroup, DownloadVariant};
use crate::state::DBPool;

#[derive(Debug, FromRow)]
struct BatchEntity {
    id: Uuid,
    #[sqlx(rename = "provider")]
    _provider: String,
    title: String,
    start_index: i32,
    end_index: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct BatchQueryOptions {
    pub title: Option<String>,
}

pub(super) async fn insert<'e, E>(
    executor: E,
    title: &str,
    range: &RangeInclusive<u32>,
    created_at: &DateTime<Utc>,
    updated_at: &DateTime<Utc>,
) -> anyhow::Result<Uuid>
where
    E: Executor<'e, Database = Postgres>,
{
    let record = sqlx::query_file!(
        "queries/batch/insert_batch_download.sql",
        "SubsPlease",
        title,
        *range.start() as i32,
        *range.end() as i32,
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
    range: &RangeInclusive<u32>,
) -> anyhow::Result<Option<SingleResult>>
where
    E: Executor<'e, Database = Postgres>,
{
    let record = sqlx::query_file_as!(
        RawSingleResult,
        "queries/batch/query_batch_download_by_unique.sql",
        "SubsPlease",
        title,
        *range.start() as i32,
        *range.end() as i32,
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
        "queries/batch/update_batch_download_updated_at.sql",
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
    range: &RangeInclusive<u32>,
    created_at: &DateTime<Utc>,
    updated_at: &DateTime<Utc>,
) -> anyhow::Result<(Uuid, Vec<u16>)>
where
    C: Connection<Database = Postgres>,
{
    let mut transaction = conn.begin().await?;
    if let Some(record) = get_by_unique_index(&mut *transaction, title, range).await? {
        if record.updated_at < *updated_at {
            update(&mut *transaction, record.id, updated_at).await?;
        }
        transaction.commit().await?;
        return Ok((record.id, record.resolutions));
    }
    let id = insert(&mut *transaction, title, range, created_at, updated_at).await?;
    transaction.commit().await?;
    Ok((id, Vec::new()))
}

pub async fn get_with_downloads(
    conn: DBPool,
    options: Option<BatchQueryOptions>,
) -> anyhow::Result<Vec<DownloadGroup>> {
    let mut transaction = conn.begin().await?;
    let rows = get_data_batches(&mut *transaction, options).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let batch_ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let mut downloads = download::get_for_batches(&mut *transaction, &batch_ids).await?;
    transaction.commit().await?;

    let result: anyhow::Result<Vec<_>> = rows
        .into_iter()
        .map(|r| {
            Ok(DownloadGroup {
                title: r.title,
                variant: DownloadVariant::Batch((r.start_index as u32)..=(r.end_index as u32)),
                created_at: r.created_at,
                updated_at: r.updated_at,
                downloads: downloads.remove(&r.id).unwrap_or_default(),
            })
        })
        .collect();
    let mut batchs = result?;
    batchs.sort_by_key(|ep| Reverse(ep.updated_at));
    Ok(batchs)
}

async fn get_data_batches<'e, E>(
    executor: E,
    options: Option<BatchQueryOptions>,
) -> anyhow::Result<Vec<BatchEntity>>
where
    E: Executor<'e, Database = Postgres>,
{
    let mut qb = QueryBuilder::new("SELECT * FROM batch_download");
    if let Some(options) = options {
        if let Some(title) = options.title {
            qb.push(" WHERE title ILIKE ").push_bind(title);
        }
    }
    let query = qb
        .push(" ORDER BY updated_at DESC")
        .push(" LIMIT 25")
        .build_query_as::<BatchEntity>();

    let rows = query.fetch_all(executor).await?;
    Ok(rows)
}
