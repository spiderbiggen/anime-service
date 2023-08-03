use anime_service::{jobs::poller, state::AppState};
use anyhow::Result;
use chrono::{Utc, Duration};
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_state = AppState::new()?;
    sqlx::migrate!().run(&app_state.pool).await?;
    let job = poller::PersistentPoller::new_with_last_update(
        app_state.clone(),
        Utc::now() - Duration::hours(7 * 24),
    )?;
    poller::start(job);
    anime_service::serve_combined(app_state).await?;
    Ok(())
}