use std::default::Default;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use tokio::sync::broadcast::Sender;
use tokio::task::JoinHandle;
use tokio::time::{interval_at, timeout, Instant, Interval, MissedTickBehavior};
use tracing::{debug, info, instrument, trace};

use datasource::repository;

use crate::datasource;
use crate::models::DownloadGroup;
use crate::state::{AppState, DBPool, ReqwestClient};

const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub trait NewDownloadsHandler: Sized + Send + Sync {
    fn handle_new_downloads(
        &self,
        groups: Vec<DownloadGroup>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}

pub struct Poller<Handler: NewDownloadsHandler> {
    client: ReqwestClient,
    downloads_handler: Handler,
    last_update: Arc<Mutex<DateTime<Utc>>>,
}

impl Poller<PersistentPoller> {
    pub async fn persistent_from_state(state: &AppState) -> anyhow::Result<Self> {
        let last_update = repository::downloads::last_updated(&state.pool)
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
        Self::new_with_last_updated_at(client, handler, DateTime::default())
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
                self.tick().await;
            }
        })
    }

    #[instrument(skip(self))]
    async fn tick(&self) {
        let last_updated_at = *self.last_update.lock().unwrap();
        if let Ok(last_updated_at) = self.poll_nyaa(last_updated_at).await {
            *self.last_update.lock().unwrap() = last_updated_at;
        };
    }

    #[instrument(skip(self))]
    async fn poll_nyaa(&self, last_update: DateTime<Utc>) -> anyhow::Result<DateTime<Utc>> {
        trace!("fetching anime downloads");
        let groups = get_groups(&self.client).await?;
        let filtered_groups: Vec<_> = groups.filter(|g| g.updated_at > last_update).collect();
        if filtered_groups.is_empty() {
            debug!("Found no new downloads");
            return Ok(last_update);
        }

        let count = filtered_groups.len();
        let last_update = filtered_groups
            .iter()
            .map(|g| g.updated_at)
            .max()
            .unwrap_or(last_update);
        self.downloads_handler
            .handle_new_downloads(filtered_groups)
            .await?;
        info!("processed {count} groups");
        Ok(last_update)
    }
}

#[instrument(skip_all, err)]
async fn get_groups(
    client: &reqwest::Client,
) -> anyhow::Result<impl Iterator<Item = DownloadGroup>> {
    let groups_future = nyaa::groups(client, None);
    let groups = timeout(Duration::from_secs(10), groups_future).await??;
    let result = groups.into_iter().map(Into::into);
    Ok(result)
}

fn interval_at_next_period(period: Duration) -> anyhow::Result<Interval> {
    let start = Instant::now();
    let now: DateTime<Utc> = Utc::now();
    let seconds = now.timestamp();
    let remaining_seconds =
        period.as_secs().cast_signed() - (seconds % period.as_secs().cast_signed());
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
    #[must_use]
    pub fn new(sender: Sender<DownloadGroup>) -> Self {
        Self { sender }
    }
}

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
    #[must_use]
    pub fn new(state: &AppState) -> Self {
        Self {
            database: state.pool.clone(),
            sender: state.downloads_channel.clone(),
        }
    }

    async fn save_downloads(&self, groups: &[DownloadGroup]) -> anyhow::Result<()> {
        repository::downloads::insert_groups(self.database.clone(), groups).await?;
        Ok(())
    }
}

impl NewDownloadsHandler for PersistentPoller {
    async fn handle_new_downloads(&self, groups: Vec<DownloadGroup>) -> anyhow::Result<()> {
        self.save_downloads(&groups).await?;
        for group in groups {
            let _ = self.sender.send(group);
        }
        Ok(())
    }
}
