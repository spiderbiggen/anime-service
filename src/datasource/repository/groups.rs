use chrono::{DateTime, Utc};
use sqlx::types::Uuid;
use sqlx::{query_file, Connection, Executor, Pool, Postgres};

use crate::datasource::repository::{batch, download, episode, movie};
use crate::models::{DownloadGroup, DownloadVariant};

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

pub async fn upsert_group<C>(conn: &mut C, group: &DownloadGroup) -> anyhow::Result<Uuid>
where
    C: Connection<Database = Postgres>,
{
    let mut transaction = conn.begin().await?;
    let id = match &group.variant {
        DownloadVariant::Batch(range) => {
            let (id, resolutions) = batch::upsert(
                &mut *transaction,
                &group.title,
                range,
                &group.created_at,
                &group.updated_at,
            )
            .await?;
            for download in &group.downloads {
                if resolutions.contains(&download.resolution) {
                    continue;
                }
                download::insert_batch(&mut *transaction, id, download).await?;
            }
            id
        }
        DownloadVariant::Episode(ep) => {
            let (id, resolutions) = episode::upsert(
                &mut *transaction,
                &group.title,
                ep,
                &group.created_at,
                &group.updated_at,
            )
            .await?;
            for download in &group.downloads {
                if resolutions.contains(&download.resolution) {
                    continue;
                }
                download::insert_episode(&mut *transaction, id, download).await?;
            }
            id
        }
        DownloadVariant::Movie => {
            let (id, resolutions) = movie::upsert(
                &mut *transaction,
                &group.title,
                &group.created_at,
                &group.updated_at,
            )
            .await?;
            for download in &group.downloads {
                if resolutions.contains(&download.resolution) {
                    continue;
                }
                download::insert_movie(&mut *transaction, id, download).await?;
            }
            id
        }
    };
    transaction.commit().await?;
    Ok(id)
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
