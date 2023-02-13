use std::time::Duration;

use anyhow::Result;
use tokio::task::JoinHandle;
use tracing::log::{error, warn};

use datasource::repository;

use crate::datasource;
use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;
use crate::state::{DBPool, HyperClient};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub(crate) fn start(
    client: HyperClient,
    pool: DBPool,
    cache: RequestCache<Vec<DownloadGroup>>,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let poller = Poller::new(client, pool, cache);
        poller.run().await;
    })
}

#[derive(Debug, Clone)]
pub(crate) struct Poller {
    client: HyperClient,
    database: DBPool,
    cache: RequestCache<Vec<DownloadGroup>>,
    interval: Duration,
}

impl Poller {
    pub fn new(
        client: HyperClient,
        database: DBPool,
        cache: RequestCache<Vec<DownloadGroup>>,
    ) -> Self {
        Self {
            client,
            database,
            cache,
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

    async fn get_groups(&self) -> Result<Vec<DownloadGroup>> {
        let result: Vec<DownloadGroup> = nyaa::groups(self.client.clone(), "")
            .await?
            .into_iter()
            .map(|e| e.into())
            .collect();
        if let Some(last_update) = result.iter().map(|a| a.episode.updated_at).max() {
            self.cache.invalidate_if_newer("", last_update)
        }
        Ok(result)
    }

    async fn save_downloads(&self, group: DownloadGroup) -> Result<()> {
        let record = repository::episode::upsert(self.database.clone(), &group.episode).await?;

        for download in group.downloads.iter() {
            if let Some(v) = record.resolutions.as_ref() {
                if v.contains(&download.resolution) {
                    continue;
                }
            }
            let insert =
                repository::download::insert(self.database.clone(), &record.id, download).await;

            if let Err(err) = insert {
                warn!("Failed to save download[{download:?}]: {err}")
            }
        }

        self.cache
            .invalidate_if_newer(group.episode.title, group.episode.updated_at);
        Ok(())
    }
}
