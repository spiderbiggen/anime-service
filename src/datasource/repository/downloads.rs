use std::cmp::Reverse;

use anyhow::Context;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use sqlx::types::Uuid;
use sqlx::{query_file, Connection, Executor, Pool, Postgres};

use crate::datasource::repository::download_resolutions;
use crate::models::{DownloadGroup, DownloadVariant, Episode};

pub mod batch;
pub mod episode;
pub mod movie;

// TODO: remove default and provide dynamically
const PROVIDER_DEFAULT: &str = "SubsPlease";

#[derive(Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "download_variant", rename_all = "lowercase")]
pub enum Variant {
    Batch,
    Episode,
    Movie,
}

pub async fn insert_groups(
    executor: Pool<Postgres>,
    groups: &[DownloadGroup],
) -> anyhow::Result<Vec<Uuid>> {
    let mut transaction = executor.begin().await?;
    let mut ids = Vec::with_capacity(groups.len());
    for group in groups {
        ids.push(upsert_group(&mut *transaction, group).await?);
    }
    transaction.commit().await?;
    Ok(ids)
}

#[derive(Debug, Default)]
pub struct QueryOptions {
    pub title: Option<String>,
}

pub async fn get_with_downloads(
    executor: Pool<Postgres>,
    variant: Option<Variant>,
    options: Option<QueryOptions>,
) -> anyhow::Result<Vec<DownloadGroup>> {
    let mut transaction = executor.begin().await?;
    let rows = get_data_episodes(&mut *transaction, variant, options.as_ref()).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let episode_ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let mut downloads =
        download_resolutions::resolutions_for_downloads(&mut *transaction, &episode_ids).await?;
    transaction.commit().await?;

    let result: anyhow::Result<Vec<_>> = rows
        .into_iter()
        .map(|r| {
            Ok(DownloadGroup {
                title: r.title,
                variant: match r.variant {
                    Variant::Batch => {
                        let start = r
                            .start_index
                            .context("expected a `start_index` for the batch variant")?;
                        let end = r
                            .end_index
                            .context("expected a `end_index` for the batch variant")?;
                        DownloadVariant::Batch(start..=end)
                    }
                    Variant::Episode => DownloadVariant::Episode(Episode {
                        episode: r
                            .episode
                            .context("Expected an episode number for the `episode` variant")?,
                        decimal: r.decimal,
                        version: r.version,
                        extra: r.extra,
                    }),
                    Variant::Movie => DownloadVariant::Movie,
                },
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

pub async fn upsert_group<C>(conn: &mut C, group: &DownloadGroup) -> anyhow::Result<Uuid>
where
    C: Connection<Database = Postgres>,
{
    let mut transaction = conn.begin().await?;
    let (id, resolutions) = match &group.variant {
        DownloadVariant::Batch(range) => {
            batch::upsert(
                &mut *transaction,
                &group.title,
                range,
                &group.created_at,
                &group.updated_at,
            )
            .await?
        }
        DownloadVariant::Episode(episode) => {
            episode::upsert(
                &mut *transaction,
                &group.title,
                episode,
                &group.created_at,
                &group.updated_at,
            )
            .await?
        }
        DownloadVariant::Movie => {
            movie::upsert(
                &mut *transaction,
                &group.title,
                &group.created_at,
                &group.updated_at,
            )
            .await?
        }
    };
    for download in &group.downloads {
        if resolutions.contains(&download.resolution) {
            continue;
        }
        download_resolutions::insert(&mut *transaction, id, download).await?;
    }

    transaction.commit().await?;
    Ok(id)
}

pub(super) async fn update_download<'e, E>(
    executor: E,
    id: Uuid,
    updated_at: &DateTime<Utc>,
) -> anyhow::Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    query_file!("queries/update_download_updated_at.sql", id, updated_at)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn last_updated<'e, E>(executor: E) -> anyhow::Result<Option<DateTime<Utc>>>
where
    E: Executor<'e, Database = Postgres>,
{
    let record = query_file!("queries/query_last_updated_at.sql")
        .fetch_one(executor)
        .await?;
    Ok(record.updated_at)
}

struct DownloadEntity {
    id: Uuid,
    _provider: String,
    title: String,
    episode: Option<u32>,
    decimal: Option<u32>,
    version: Option<u32>,
    start_index: Option<u32>,
    end_index: Option<u32>,
    extra: Option<String>,
    variant: Variant,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn get_data_episodes<'e, E>(
    executor: E,
    variant: Option<Variant>,
    options: Option<&QueryOptions>,
) -> anyhow::Result<Vec<DownloadEntity>>
where
    E: Executor<'e, Database = Postgres>,
{
    let query = query_file!(
        "queries/query_downloads_by_title.sql",
        variant as _,
        options.and_then(|o| o.title.as_ref())
    );
    let mut stream = query.fetch(executor);
    let mut rows = Vec::with_capacity(25);
    while let Some(row) = stream.next().await {
        let record = row?;
        rows.push(DownloadEntity {
            id: record.id,
            _provider: record.provider,
            title: record.title,
            episode: record.episode.map(|v| v as u32),
            decimal: record.decimal.map(|v| v as u32),
            version: record.version.map(|v| v as u32),
            start_index: record.start_index.map(|v| v as u32),
            end_index: record.end_index.map(|v| v as u32),
            extra: record.extra,
            variant: record.variant,
            created_at: record.created_at,
            updated_at: record.updated_at,
        });
    }
    Ok(rows)
}

struct RawSingleDownloadResult {
    id: Uuid,
    updated_at: DateTime<Utc>,
    resolutions: Option<Vec<i16>>,
}

struct SingleDownloadResult {
    id: Uuid,
    updated_at: DateTime<Utc>,
    resolutions: Vec<u16>,
}

impl From<RawSingleDownloadResult> for SingleDownloadResult {
    fn from(value: RawSingleDownloadResult) -> Self {
        Self {
            id: value.id,
            updated_at: value.updated_at,
            resolutions: value
                .resolutions
                .into_iter()
                .flatten()
                .map(|res| res as u16)
                .collect(),
        }
    }
}
