use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use jiff::{SignedDuration, Timestamp};
use serde::Deserialize;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinHandle;
use tokio::time::{Instant, Interval, MissedTickBehavior, interval_at, timeout};
use tracing::{debug, info, instrument, trace};

use datasource::repository;

use crate::datasource;
use crate::jobs::resolver::ShowResolver;
use crate::models::DownloadGroup;
use crate::state::{AppState, DBPool, ReqwestClient};

const DEFAULT_INTERVAL: Duration = Duration::from_mins(5);

/// Optional overrides for how the poller runs, read from `POLL_*` environment variables.
///
/// They exist for testing a consumer without waiting for Nyaa to publish something: a short
/// interval plus a lookback replays recent releases within a minute.
#[derive(Debug, Default, Deserialize)]
pub struct PollerConfig {
    /// `POLL_INTERVAL_SECS`: seconds between polls, 300 by default. Polls are aligned to
    /// multiples of the interval.
    interval_secs: Option<u64>,
    /// `POLL_LOOKBACK_SECS`: replay the releases published within this many seconds on the first
    /// poll, instead of continuing after the newest stored release.
    lookback_secs: Option<u64>,
}

impl PollerConfig {
    /// Reads the configuration from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error if a `POLL_*` variable is not a valid number.
    pub fn from_env() -> anyhow::Result<Self> {
        envy::prefixed("POLL_")
            .from_env()
            .context("POLL_INTERVAL_SECS and POLL_LOOKBACK_SECS must be whole numbers of seconds")
    }

    /// How often to poll.
    ///
    /// # Errors
    ///
    /// Returns an error if the interval is zero.
    pub fn interval(&self) -> anyhow::Result<Duration> {
        match self.interval_secs {
            None => Ok(DEFAULT_INTERVAL),
            Some(0) => bail!("POLL_INTERVAL_SECS must be at least 1"),
            Some(secs) => Ok(Duration::from_secs(secs)),
        }
    }

    /// The moment to replay releases from, when a lookback is configured.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookback is too large to represent.
    pub fn replay_since(&self) -> anyhow::Result<Option<Timestamp>> {
        self.lookback_secs
            .map(|secs| {
                let secs = i64::try_from(secs).context("POLL_LOOKBACK_SECS is too large")?;
                Timestamp::now()
                    .checked_sub(SignedDuration::from_secs(secs))
                    .context("POLL_LOOKBACK_SECS reaches back too far")
            })
            .transpose()
    }
}

pub trait NewDownloadsHandler: Sized + Send + Sync {
    fn handle_new_downloads(
        &self,
        groups: Vec<DownloadGroup>,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}

pub struct Poller<Handler: NewDownloadsHandler> {
    client: ReqwestClient,
    downloads_handler: Handler,
    last_update: Arc<Mutex<Timestamp>>,
}

impl Poller<PersistentPoller> {
    /// Creates a poller that stores and broadcasts new releases.
    ///
    /// It continues after the newest stored release, or replays from `replay_since` when given.
    pub async fn persistent_from_state(
        state: &AppState,
        replay_since: Option<Timestamp>,
    ) -> anyhow::Result<Self> {
        let last_update = match replay_since {
            Some(since) => since,
            None => repository::downloads::last_updated(&state.pool)
                .await?
                .unwrap_or_else(Timestamp::now),
        };
        Ok(Self::new_with_last_updated_at(
            state.client.clone(),
            PersistentPoller::new(state),
            last_update,
        ))
    }
}

impl<Handler: NewDownloadsHandler + 'static> Poller<Handler> {
    pub fn new(client: ReqwestClient, handler: Handler) -> Self {
        Self::new_with_last_updated_at(client, handler, Timestamp::UNIX_EPOCH)
    }

    pub fn new_with_last_updated_at(
        client: ReqwestClient,
        handler: Handler,
        last_updated_at: Timestamp,
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
        }
    }

    #[instrument(skip(self))]
    async fn poll_nyaa(&self, last_update: Timestamp) -> anyhow::Result<Timestamp> {
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
    let now = Timestamp::now();
    let seconds = now.as_second();
    let period_seconds = period.as_secs().cast_signed();
    let remaining_seconds = period_seconds - (seconds % period_seconds);
    let next_period = Timestamp::from_second(seconds + remaining_seconds)?;
    let offset = now.duration_until(next_period).unsigned_abs();
    Ok(interval_at(start + offset, period))
}

#[derive(Debug)]
pub struct PersistentPoller {
    database: DBPool,
    sender: Sender<DownloadGroup>,
    resolver: ShowResolver,
}

impl PersistentPoller {
    #[must_use]
    pub fn new(state: &AppState) -> Self {
        Self {
            database: state.pool.clone(),
            sender: state.downloads_channel.clone(),
            resolver: ShowResolver::new(state.tmdb.clone(), state.pool.clone()),
        }
    }

    async fn save_downloads(&self, groups: &[DownloadGroup]) -> anyhow::Result<()> {
        repository::downloads::insert_groups(self.database.clone(), groups).await?;
        Ok(())
    }
}

impl NewDownloadsHandler for PersistentPoller {
    async fn handle_new_downloads(&self, mut groups: Vec<DownloadGroup>) -> anyhow::Result<()> {
        self.save_downloads(&groups).await?;
        self.resolver.attach(&mut groups).await;
        for group in groups {
            let _ = self.sender.send(group);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(vars: &[(&str, &str)]) -> Result<PollerConfig, envy::Error> {
        envy::prefixed("POLL_").from_iter(
            vars.iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned())),
        )
    }

    #[test]
    fn defaults_when_nothing_is_set() {
        let config = config(&[]).expect("valid config");

        assert_eq!(config.interval().expect("valid"), DEFAULT_INTERVAL);
        assert!(config.replay_since().expect("valid").is_none());
    }

    #[test]
    fn interval_can_be_overridden() {
        let config = config(&[("POLL_INTERVAL_SECS", "60")]).expect("valid config");

        assert_eq!(config.interval().expect("valid"), Duration::from_mins(1));
    }

    #[test]
    fn a_zero_interval_is_rejected() {
        let config = config(&[("POLL_INTERVAL_SECS", "0")]).expect("valid config");

        assert!(config.interval().is_err());
    }

    #[test]
    fn values_that_are_not_numbers_are_rejected() {
        assert!(config(&[("POLL_INTERVAL_SECS", "soon")]).is_err());
        assert!(config(&[("POLL_LOOKBACK_SECS", "-5")]).is_err());
    }

    #[test]
    fn lookback_replays_from_that_long_ago() {
        let config = config(&[("POLL_LOOKBACK_SECS", "3600")]).expect("valid config");

        let since = config
            .replay_since()
            .expect("valid")
            .expect("a lookback is set");
        let age = Timestamp::now().duration_since(since).as_secs();

        assert!((3600..3605).contains(&age), "lookback was {age}s");
    }

    #[test]
    fn a_lookback_that_is_too_large_is_rejected() {
        let huge = u64::MAX.to_string();
        let beyond_range = i64::MAX.to_string();

        for value in [&huge, &beyond_range] {
            let config = config(&[("POLL_LOOKBACK_SECS", value)]).expect("valid config");
            assert!(config.replay_since().is_err(), "{value} should be rejected");
        }
    }
}
