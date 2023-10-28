use std::time::Duration;

use anyhow::{anyhow, Result};
use axum::async_trait;
use chrono::{DateTime, Timelike, Utc};
use tokio::sync::broadcast::Sender;
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

pub async fn start_persistent(state: AppState) -> Result<JoinHandle<()>> {
    let poller = PersistentPoller::new(state).await?;
    start(poller)
}

#[async_trait]
pub trait Poller: Sized + Send + Sync {
    fn reqwest_client(&self) -> reqwest::Client;
    fn last_update(&self) -> DateTime<Utc>;

    async fn handle_group(&self, group: DownloadGroup) -> Result<()>;
    async fn handle_last_updated(&mut self, updated: DateTime<Utc>) -> Result<()>;
}

pub fn start(poller: impl Poller + 'static) -> Result<JoinHandle<()>> {
    start_with_period(poller, DEFAULT_INTERVAL)
}

pub fn start_with_period(
    poller: impl Poller + 'static,
    period: Duration,
) -> Result<JoinHandle<()>> {
    let interval = interval_at_next_minute(period)?;
    Ok(start_with_interval(poller, interval))
}

pub fn start_with_interval(
    poller: impl Poller + 'static,
    mut interval: Interval,
) -> JoinHandle<()> {
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    tokio::task::spawn(async move {
        let mut poller = poller;
        loop {
            interval.tick().await;
            if let Err(err) = execute(&mut poller).await {
                error!("failed to refresh anime downloads: {err}")
            }
        }
    })
}

async fn get_groups(client: reqwest::Client) -> Result<Vec<DownloadGroup>> {
    let result: Vec<DownloadGroup> = nyaa::groups(client, None)
        .await?
        .into_iter()
        .map(|e| e.into())
        .collect();
    Ok(result)
}

#[instrument(skip(poller))]
async fn execute(poller: &mut impl Poller) -> Result<()> {
    trace!("fetching anime downloads");
    let mut groups = get_groups(poller.reqwest_client()).await?;
    groups.sort_by_key(|g| g.episode.updated_at);
    let last_update = poller.last_update();
    let iter = groups
        .into_iter()
        .skip_while(|g| g.episode.updated_at <= last_update);

    let mut count = 0;
    let mut updated = last_update;
    for group in iter {
        updated = group.episode.updated_at;
        poller.handle_group(group).await?;
        count += 1;
    }
    poller.handle_last_updated(updated).await?;
    trace!("processed {count} groups");
    Ok(())
}

fn interval_at_next_minute(period: Duration) -> Result<Interval> {
    let now: DateTime<Utc> = Utc::now();
    let minute = (now + chrono::Duration::minutes(1))
        .with_second(0)
        .and_then(|t| t.with_nanosecond(0))
        .ok_or(anyhow!("failed to strip seconds"))?;
    let duration = (minute - now).to_std()?;
    let start = Instant::now() + duration;
    Ok(interval_at(start, period))
}

#[derive(Debug)]
pub struct TransientPoller {
    client: ReqwestClient,
    sender: Sender<DownloadGroup>,
    last_update: DateTime<Utc>,
}

impl TransientPoller {
    pub fn new_with_last_update(
        sender: Sender<DownloadGroup>,
        last_update: DateTime<Utc>,
    ) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            sender,
            last_update,
        })
    }
}

#[async_trait]
impl Poller for TransientPoller {
    fn reqwest_client(&self) -> reqwest::Client {
        self.client.clone()
    }

    fn last_update(&self) -> DateTime<Utc> {
        self.last_update
    }

    async fn handle_group(&self, group: DownloadGroup) -> Result<()> {
        let _ = self.sender.send(group);
        Ok(())
    }

    async fn handle_last_updated(&mut self, updated: DateTime<Utc>) -> Result<()> {
        self.last_update = updated;
        Ok(())
    }
}

#[derive(Debug)]
pub struct PersistentPoller {
    client: ReqwestClient,
    database: DBPool,
    cache: RequestCache<Vec<DownloadGroup>>,
    download_channel: Sender<DownloadGroup>,
    last_update: DateTime<Utc>,
}

impl PersistentPoller {
    pub async fn new(state: AppState) -> Result<Self> {
        let last_update = repository::episode::last_update(state.pool.clone())
            .await?
            .unwrap_or_else(Utc::now);
        Self::new_with_last_update(state, last_update)
    }

    pub fn new_with_last_update(state: AppState, last_update: DateTime<Utc>) -> Result<Self> {
        Ok(Self {
            client: state.client,
            database: state.pool,
            cache: state.downloads_cache,
            download_channel: state.downloads_channel,
            last_update,
        })
    }

    async fn save_downloads(&self, group: &DownloadGroup) -> Result<()> {
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
            .invalidate_if_newer(&group.episode.title, group.episode.updated_at);
        Ok(())
    }
}

#[async_trait]
impl Poller for PersistentPoller {
    fn reqwest_client(&self) -> reqwest::Client {
        self.client.clone()
    }

    fn last_update(&self) -> DateTime<Utc> {
        self.last_update
    }

    async fn handle_group(&self, group: DownloadGroup) -> Result<()> {
        self.save_downloads(&group).await?;
        let _ = self.download_channel.send(group);
        Ok(())
    }

    async fn handle_last_updated(&mut self, updated: DateTime<Utc>) -> Result<()> {
        self.cache.invalidate_if_newer("", updated);
        self.last_update = updated;
        Ok(())
    }
}
