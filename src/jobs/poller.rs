use std::time::Duration;

use anyhow::Result;
use futures::{executor, future, FutureExt};
use tokio::task::JoinHandle;
use tracing::log::{error, warn};
use tracing::{instrument, trace};

use datasource::repository;

use crate::datasource;
use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;
use crate::state::{AppState, DBPool, HyperClient};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub(crate) fn start(state: AppState) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        Poller::new(state).run().await;
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
    pub fn new(state: AppState) -> Self {
        Self {
            client: state.client,
            database: state.pool,
            cache: state.downloads_cache,
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

    #[instrument(skip(self))]
    async fn execute(&self) -> Result<()> {
        trace!("fetching anime downloads");
        let groups = self.get_groups().await?;
        let group_size = groups.len();
        trace!("found {group_size} groups");
        let mut futures = Vec::with_capacity(group_size);
        for group in groups {
            futures.push(self.save_downloads(group));
        }
        future::try_join_all(futures).await?;
        trace!("saved {group_size} groups");
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
