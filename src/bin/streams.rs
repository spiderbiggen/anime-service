use anime_service::jobs::poller;
use anyhow::Result;
use chrono::{Utc, Duration};
use tokio::sync::broadcast;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let (tx, _) = broadcast::channel(32);
    let job = poller::TransientPoller::new_with_last_update(tx.clone(), Utc::now() - Duration::hours(7 * 24))?;
    poller::start(job)?;

    anime_service::serve_tonic(tx).await?;
    Ok(())
}
