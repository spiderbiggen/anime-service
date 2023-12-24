use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, Postgres};

use super::{update_download, RawSingleDownloadResult, SingleDownloadResult, PROVIDER_DEFAULT};

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
            update_download(&mut *transaction, record.id, updated_at).await?;
        }
        transaction.commit().await?;
        return Ok((record.id, record.resolutions));
    }
    let id = insert(&mut *transaction, title, created_at, updated_at).await?;
    transaction.commit().await?;
    Ok((id, Vec::new()))
}

async fn get_by_unique_index<'e, E>(
    executor: E,
    title: &str,
) -> anyhow::Result<Option<SingleDownloadResult>>
where
    E: Executor<'e, Database = Postgres>,
{
    let record = sqlx::query_file_as!(
        RawSingleDownloadResult,
        "queries/movie/query_movie_download_by_unique.sql",
        PROVIDER_DEFAULT,
        title,
    )
    .fetch_optional(executor)
    .await?
    .map(|record| record.into());
    Ok(record)
}

async fn insert<'e, E>(
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
        PROVIDER_DEFAULT,
        title,
        created_at,
        updated_at,
    )
    .fetch_one(executor)
    .await?;
    Ok(record.id)
}
