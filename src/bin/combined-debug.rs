use anyhow::Result;
use chrono::{Duration, Utc};
use tracing_subscriber::prelude::*;

use anime_service::{jobs::poller, state::AppState};
use poller::{PersistentPoller, Poller};

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_state = AppState::new()?;
    sqlx::migrate!().run(&app_state.pool).await?;

    let one_week = Duration::try_weeks(1).expect("1 week fits in a duration");
    let last_updated_at = Utc::now() - one_week;
    let handler = PersistentPoller::new(&app_state);
    let poller =
        Poller::new_with_last_updated_at(app_state.client.clone(), handler, last_updated_at);

    let interval = tokio::time::interval_at(
        (std::time::Instant::now() + std::time::Duration::from_secs(2)).into(),
        std::time::Duration::from_secs(60),
    );
    poller.start_with_interval(interval);
    anime_service::serve_combined(app_state).await?;
    Ok(())
}
