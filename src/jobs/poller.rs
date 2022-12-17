use crate::{models, HyperClient};
use anyhow::{bail, Result};
use sqlx::types::Uuid;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::log::{error, warn};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub(crate) fn start(client: HyperClient, pool: Pool<Postgres>) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let poller = Poller::new(client, pool);
        poller.run().await;
    })
}

#[derive(Debug, Clone)]
pub(crate) struct Poller {
    client: HyperClient,
    database: Pool<Postgres>,
    interval: Duration,
}

impl Poller {
    pub fn new(client: HyperClient, database: Pool<Postgres>) -> Self {
        Self {
            client,
            database,
            interval: DEFAULT_INTERVAL,
        }
    }

    pub async fn run(&self) {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            interval.tick().await;
            if let Err(err) = self.execute().await {
                error!("failed to refresh anime downloads: {err}")
            }
        }
    }

    async fn execute(&self) -> Result<()> {
        let groups = self.get_groups().await?;
        for group in groups {
            self.save_downloads(group).await?
        }
        Ok(())
    }

    async fn get_groups(&self) -> Result<Vec<models::DownloadGroup>> {
        let result = nyaa::groups(self.client.clone(), "")
            .await?
            .into_iter()
            .map(|e| e.into())
            .collect();
        Ok(result)
    }

    async fn save_downloads(&self, group: models::DownloadGroup) -> Result<()> {
        struct Record {
            id: Uuid,
            resolutions: Option<Vec<String>>,
        }

        let mut record = sqlx::query_file_as!(
            Record,
            "queries/query_episode_download_by_unique.sql",
            Option::<String>::None,
            group.episode.title,
            group.episode.episode.map(|e| e as i32),
            group.episode.decimal.map(|e| e as i32),
            group.episode.version.map(|e| e as i32)
        )
        .fetch_optional(&self.database)
        .await?;

        if record.is_none() {
            let id = sqlx::query_file!(
                "queries/insert_episode_download.sql",
                group.episode.title,
                group.episode.episode.map(|e| e as i32),
                group.episode.decimal.map(|e| e as i32),
                group.episode.version.map(|e| e as i32),
                group.episode.pub_date,
            )
            .fetch_one(&self.database)
            .await?
            .id;
            record = Some(Record {
                id,
                resolutions: None,
            });
        }

        match record {
            None => bail!("couldn't find or store download group in database for {group:?}"),
            Some(r) => {
                for download in group.downloads.iter() {
                    if let Some(v) = r.resolutions.as_ref() {
                        if v.contains(&download.resolution) {
                            continue;
                        }
                    }
                    let insert = sqlx::query_file!(
                        "queries/insert_episode_download_resolution.sql",
                        r.id,
                        download.resolution,
                        download.torrent,
                        Some(&download.file_name),
                        download.comments,
                        Option::<String>::None,
                        download.pub_date,
                    )
                    .execute(&self.database)
                    .await;
                    if let Err(err) = insert {
                        warn!("Failed to save download[{download:?}]: {err}")
                    }
                }
            }
        }
        Ok(())
    }
}
