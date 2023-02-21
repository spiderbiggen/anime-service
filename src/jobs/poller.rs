use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{Timelike, Utc};
use futures::future;
use tokio::task::JoinHandle;
use tokio::time::{interval_at, Instant, Interval, MissedTickBehavior};
use tracing::log::{error, warn};
use tracing::{instrument, trace};

use datasource::repository;

use crate::datasource;
use crate::models::DownloadGroup;
use crate::request_cache::RequestCache;
use crate::state::{AppState, DBPool, ReqwestClient};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);

pub(crate) fn start(state: AppState) -> Result<JoinHandle<()>> {
    let mut poller = Poller::new(state)?;
    let handle = tokio::task::spawn(async move {
        poller.run().await;
    });
    Ok(handle)
}

#[derive(Debug)]
pub(crate) struct Poller {
    client: ReqwestClient,
    database: DBPool,
    cache: RequestCache<Vec<DownloadGroup>>,
    interval: Interval,
}

impl Poller {
    pub fn new(state: AppState) -> Result<Self> {
        let now = Utc::now();
        let minute = now
            .with_minute(now.minute() + 1)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .ok_or(anyhow!("failed to strip seconds"))?;
        let duration = (minute - now).to_std()?;
        let instant = Instant::now() + duration;
        let mut interval = interval_at(instant, DEFAULT_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        Ok(Self {
            client: state.client,
            database: state.pool,
            cache: state.downloads_cache,
            interval,
        })
    }

    pub async fn run(&mut self) {
        loop {
            self.interval.tick().await;
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
        let mut futures = Vec::with_capacity(group_size);
        for group in groups {
            futures.push(self.save_downloads(group));
        }
        future::try_join_all(futures).await?;
        trace!("saved {group_size} groups");
        Ok(())
    }

    async fn get_groups(&self) -> Result<Vec<DownloadGroup>> {
        let result: Vec<DownloadGroup> = nyaa::groups(self.client.clone(), None)
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