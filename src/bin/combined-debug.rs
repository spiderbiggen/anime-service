use anyhow::Result;
use std::time::{Duration, Instant};
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

    let poller = get_poller(&app_state);

    let interval = tokio::time::interval_at(
        (Instant::now() + Duration::from_secs(2)).into(),
        Duration::from_mins(1),
    );
    poller.start_with_interval(interval);
    anime_service::serve_combined(app_state).await?;
    Ok(())
}

fn get_poller(app_state: &AppState) -> Poller<PersistentPoller> {
    use chrono::{Duration, Utc};

    let one_week = Duration::weeks(1);
    let last_updated_at = Utc::now() - one_week;
    let handler = PersistentPoller::new(app_state);
    Poller::new_with_last_updated_at(app_state.client.clone(), handler, last_updated_at)
}
