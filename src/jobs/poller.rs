use std::default::Default;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use axum::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{interval_at, Instant, Interval, MissedTickBehavior};
use tracing::{error, instrument, trace};

use datasource::repository;

use crate::datasource;
use crate::models::DownloadGroup;
use crate::state::{AppState, DBPool, ReqwestClient};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[async_trait]
pub trait NewDownloadsHandler: Sized + Send + Sync {
    async fn handle_new_downloads(&self, groups: Vec<DownloadGroup>) -> anyhow::Result<()>;
}

pub struct Poller<Handler: NewDownloadsHandler> {
    client: ReqwestClient,
    downloads_handler: Handler,
    last_update: Arc<Mutex<DateTime<Utc>>>,
}

impl Poller<PersistentPoller> {
    pub async fn persistent_from_state(state: &AppState) -> anyhow::Result<Self> {
        let last_update = repository::groups::last_updated(&state.pool)
            .await?
            .unwrap_or_else(Utc::now);
        Ok(Self::new_with_last_updated_at(
            state.client.clone(),
            PersistentPoller::new(state),
            last_update,
        ))
    }
}

impl<Handler: NewDownloadsHandler + 'static> Poller<Handler> {
    pub fn new(client: ReqwestClient, handler: Handler) -> Self {
        Self::new_with_last_updated_at(client, handler, Default::default())
    }

    pub fn new_with_last_updated_at(
        client: ReqwestClient,
        handler: Handler,
        last_updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            client,
            downloads_handler: handler,
            last_update: Arc::new(Mutex::new(last_updated_at)),
        }
    }

    pub fn start(self) -> anyhow::Result<JoinHandle<()>> {
        self.start_with_period(DEFAULT_INTERVAL)
    }

    pub fn start_with_period(self, period: Duration) -> anyhow::Result<JoinHandle<()>> {
        let interval = interval_at_next_period(period)?;
        Ok(self.start_with_interval(interval))
    }

    pub fn start_with_interval(self, mut interval: Interval) -> JoinHandle<()> {
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        tokio::task::spawn(async move {
            loop {
                interval.tick().await;
                match self.execute(*self.last_update.lock().await).await {
                    Ok(update) => *self.last_update.lock().await = update,
                    Err(err) => error!("failed to refresh anime downloads: {err}"),
                }
            }
        })
    }

    #[instrument(skip_all)]
    async fn execute(&self, last_update: DateTime<Utc>) -> anyhow::Result<DateTime<Utc>> {
        trace!("fetching anime downloads");
        let mut groups = get_groups(&self.client).await?;
        groups.sort_by_key(|g| g.updated_at);
        let groups: Vec<_> = groups
            .into_iter()
            .skip_while(|g| g.updated_at <= last_update)
            .collect();
        if groups.is_empty() {
            return Ok(last_update);
        }

        let count = groups.len();
        let last_update = groups
            .iter()
            .map(|g| g.updated_at)
            .max()
            .unwrap_or(last_update);
        self.downloads_handler.handle_new_downloads(groups).await?;
        trace!("processed {count} groups");
        Ok(last_update)
    }
}

async fn get_groups(client: &reqwest::Client) -> anyhow::Result<Vec<DownloadGroup>> {
    let result: Vec<DownloadGroup> = nyaa::groups(client, None)
        .await?
        .into_iter()
        .map(|e| e.into())
        .collect();
    Ok(result)
}

fn interval_at_next_period(period: Duration) -> anyhow::Result<Interval> {
    let start = Instant::now();
    let now: DateTime<Utc> = Utc::now();
    let seconds = now.timestamp();
    let remaining_seconds = period.as_secs() as i64 - (seconds % period.as_secs() as i64);
    let minute = DateTime::from_timestamp(seconds + remaining_seconds, 0)
        .ok_or(anyhow!("failed to create new date time"))?;
    let offset = (minute - now).to_std()?;
    Ok(interval_at(start + offset, period))
}

#[derive(Debug)]
pub struct TransientPoller {
    sender: Sender<DownloadGroup>,
}

impl TransientPoller {
    pub fn new(sender: Sender<DownloadGroup>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl NewDownloadsHandler for TransientPoller {
    async fn handle_new_downloads(&self, groups: Vec<DownloadGroup>) -> anyhow::Result<()> {
        for group in groups {
            let _ = self.sender.send(group);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PersistentPoller {
    database: DBPool,
    sender: Sender<DownloadGroup>,
}

impl PersistentPoller {
    pub fn new(state: &AppState) -> Self {
        Self {
            database: state.pool.clone(),
            sender: state.downloads_channel.clone(),
        }
    }

    async fn save_downloads(&self, groups: &[DownloadGroup]) -> anyhow::Result<()> {
        repository::groups::insert_groups(self.database.clone(), groups).await?;
        Ok(())
    }
}

#[async_trait]
impl NewDownloadsHandler for PersistentPoller {
    async fn handle_new_downloads(&self, groups: Vec<DownloadGroup>) -> anyhow::Result<()> {
        self.save_downloads(&groups).await?;
        for group in groups {
            let _ = self.sender.send(group);
        }
        Ok(())
    }
}
