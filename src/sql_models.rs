use chrono::{DateTime, Utc};
use sqlx::types::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct EpisodeWithResolutions {
    pub episode: Episode,
    pub resolutions: Vec<Download>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Episode {
    pub id: Uuid,
    pub title: String,
    pub episode: Option<i32>,
    pub decimal: Option<i32>,
    pub version: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Download {
    pub episode_download_id: Uuid,
    pub resolution: String,
    pub torrent: String,
    pub file_name: String,
    pub comments: Option<String>,
    pub magnet: Option<String>,
    pub created_at: DateTime<Utc>,
}
