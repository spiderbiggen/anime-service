use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, Postgres, Transaction};

use super::{update_download, RawSingleDownloadResult, SingleDownloadResult, PROVIDER_DEFAULT};
use crate::models::Episode;

pub(super) async fn upsert<C>(
    conn: &mut C,
    title: &str,
    episode: &Episode,
    created_at: &DateTime<Utc>,
    updated_at: &DateTime<Utc>,
) -> Result<(Uuid, Vec<u16>)>
where
    C: Connection<Database = Postgres>,
{
    let mut transaction = conn.begin().await?;
    if let Some(record) = get_by_unique_index(&mut *transaction, title, episode).await? {
        if record.updated_at < *updated_at {
            update_download(&mut *transaction, record.id, updated_at).await?;
        }
        transaction.commit().await?;
        return Ok((record.id, record.resolutions));
    }

    let id = insert_episode(&mut transaction, title, episode, created_at, updated_at).await?;
    transaction.commit().await?;
    Ok((id, Vec::new()))
}

async fn get_by_unique_index<'e, E>(
    executor: E,
    title: &str,
    episode: &Episode,
) -> Result<Option<SingleDownloadResult>>
where
    E: Executor<'e, Database = Postgres>,
{
    let result = sqlx::query_file_as!(
        RawSingleDownloadResult,
        "queries/episode/query_episode_download_by_unique.sql",
        PROVIDER_DEFAULT,
        title,
        episode.episode.cast_signed(),
        episode.decimal.map(u32::cast_signed),
        episode.version.map(u32::cast_signed),
        episode.extra,
    )
    .fetch_optional(executor)
    .await?
    .map(Into::into);
    Ok(result)
}

async fn insert_episode(
    pool: &mut Transaction<'_, Postgres>,
    title: &str,
    episode: &Episode,
    created_at: &DateTime<Utc>,
    updated_at: &DateTime<Utc>,
) -> Result<Uuid> {
    let query = sqlx::query_file!(
        "queries/episode/insert_episode_download.sql",
        PROVIDER_DEFAULT,
        title,
        episode.episode.cast_signed(),
        episode.decimal.map(u32::cast_signed),
        episode.version.map(u32::cast_signed),
        episode.extra,
        created_at,
        updated_at,
    );
    Ok(query.fetch_one(&mut **pool).await?.id)
}
