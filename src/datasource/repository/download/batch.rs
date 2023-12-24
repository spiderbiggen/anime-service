use std::ops::RangeInclusive;

use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, Postgres};

use super::update_download;
use crate::datasource::repository::{RawSingleDownloadResult, SingleDownloadResult};

pub(in crate::datasource::repository) async fn insert<'e, E>(
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

pub(in crate::datasource::repository) async fn get_by_unique_index<'e, E>(
    executor: E,
    title: &str,
    range: &RangeInclusive<u32>,
) -> anyhow::Result<Option<SingleDownloadResult>>
where
    E: Executor<'e, Database = Postgres>,
{
    let record = sqlx::query_file_as!(
        RawSingleDownloadResult,
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

pub(in crate::datasource::repository) async fn upsert<C>(
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
            update_download(&mut *transaction, record.id, updated_at).await?;
        }
        transaction.commit().await?;
        return Ok((record.id, record.resolutions));
    }
    let id = insert(&mut *transaction, title, range, created_at, updated_at).await?;
    transaction.commit().await?;
    Ok((id, Vec::new()))
}
