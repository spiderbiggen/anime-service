use anyhow::Result;
use tracing_subscriber::prelude::*;

use anime_service::{jobs::poller, state::AppState};

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_state = AppState::new()?;
    sqlx::migrate!().run(&app_state.pool).await?;
    let poller_config = poller::PollerConfig::from_env()?;
    let interval = poller_config.interval()?;
    let replay_since = poller_config.replay_since()?;
    let poller = poller::Poller::persistent_from_state(&app_state, replay_since).await?;
    tracing::info!(?interval, ?replay_since, "starting the poller");
    poller.start_with_period(interval)?;

    anime_service::serve_axum(app_state).await?;
    Ok(())
}
