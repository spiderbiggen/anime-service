use anyhow::Result;
use jiff::{SignedDuration, Timestamp};
use reqwest::Client;
use tokio::sync::broadcast;
use tracing_subscriber::prelude::*;

use anime_service::jobs::poller::{Poller, TransientPoller};

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let (tx, _) = broadcast::channel(32);
    let one_week = SignedDuration::from_hours(24 * 7);
    let last_updated_at = Timestamp::now() - one_week;
    let handler = TransientPoller::new(tx.clone());
    let poller = Poller::new_with_last_updated_at(Client::default(), handler, last_updated_at);
    poller.start()?;

    anime_service::serve_tonic(tx).await?;
    Ok(())
}
