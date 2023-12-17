use std::cmp::Reverse;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use sqlx::{Connection, Executor, FromRow, Postgres, QueryBuilder, Transaction};

use crate::datasource::repository::{download, RawSingleResult, SingleResult, PROVIDER_DEFAULT};
use crate::models::{DownloadGroup, DownloadVariant, Episode};
use crate::state::DBPool;

#[derive(Debug, FromRow)]
struct EpisodeEntity {
    id: Uuid,
    #[sqlx(rename = "provider")]
    _provider: String,
    title: String,
    episode: i32,
    decimal: Option<i32>,
    version: Option<i32>,
    extra: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct EpisodeQueryOptions {
    pub title: Option<String>,
}

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
            update_episode(&mut *transaction, &record.id, updated_at).await?;
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
) -> Result<Option<SingleResult>>
where
    E: Executor<'e, Database = Postgres>,
{
    let result = sqlx::query_file_as!(
        RawSingleResult,
        "queries/episode/query_episode_download_by_unique.sql",
        PROVIDER_DEFAULT,
        title,
        episode.episode as i32,
        episode.decimal.map(|e| e as i32),
        episode.version.map(|e| e as i32),
        episode.extra,
    )
    .fetch_optional(executor)
    .await?
    .map(|record| record.into());
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
        "SubsPlease",
        title,
        episode.episode as i32,
        episode.decimal.map(|e| e as i32),
        episode.version.map(|e| e as i32),
        episode.extra,
        created_at,
        updated_at,
    );
    Ok(query.fetch_one(&mut **pool).await?.id)
}

async fn update_episode<'e, E>(executor: E, id: &Uuid, updated_at: &DateTime<Utc>) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_file!(
        "queries/episode/update_episode_download_updated_at.sql",
        id,
        updated_at,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn get_with_downloads(
    conn: DBPool,
    options: Option<EpisodeQueryOptions>,
) -> Result<Vec<DownloadGroup>> {
    let mut transaction = conn.begin().await?;
    let rows = get_data_episodes(&mut *transaction, options).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let episode_ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let mut downloads = download::get_for_episodes(&mut *transaction, &episode_ids).await?;
    transaction.commit().await?;

    let result: Result<Vec<_>> = rows
        .into_iter()
        .map(|r| {
            Ok(DownloadGroup {
                title: r.title,
                variant: DownloadVariant::Episode(Episode {
                    episode: r.episode as u32,
                    decimal: r.decimal.map(|d| d as u32),
                    version: r.version.map(|d| d as u32),
                    extra: r.extra,
                }),
                created_at: r.created_at,
                updated_at: r.updated_at,
                downloads: downloads.remove(&r.id).unwrap_or_default(),
            })
        })
        .collect();
    let mut episodes = result?;
    episodes.sort_by_key(|ep| Reverse(ep.updated_at));
    Ok(episodes)
}

async fn get_data_episodes<'e, E>(
    executor: E,
    options: Option<EpisodeQueryOptions>,
) -> Result<Vec<EpisodeEntity>>
where
    E: Executor<'e, Database = Postgres>,
{
    let mut qb = QueryBuilder::new("SELECT * FROM episode_download");
    if let Some(options) = options {
        if let Some(title) = options.title {
            qb.push(" WHERE title ILIKE ").push_bind(title);
        }
    }
    let query = qb
        .push(" ORDER BY updated_at DESC")
        .push(" LIMIT 25")
        .build_query_as::<EpisodeEntity>();

    let rows = query.fetch_all(executor).await?;
    Ok(rows)
}
