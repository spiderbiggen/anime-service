use anyhow::Result;
use chrono::{Duration, Utc};
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
    let last_updated_at = Utc::now() - Duration::hours(7 * 24);
    let handler = TransientPoller::new(tx.clone());
    let poller = Poller::new_with_last_updated_at(Default::default(), handler, last_updated_at);
    poller.start()?;

    anime_service::serve_tonic(tx).await?;
    Ok(())
}
