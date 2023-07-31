use anime_service::{jobs::poller, state::AppState};
use anyhow::Result;
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
    poller::start_persistent(app_state.clone()).await?;
    anime_service::serve_axum(app_state).await?;
    Ok(())
}
