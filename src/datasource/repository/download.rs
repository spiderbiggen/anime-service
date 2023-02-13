use std::collections::HashMap;

use anyhow::Result;
use sqlx::types::Uuid;
use sqlx::{Pool, Postgres, QueryBuilder};
use tokio_stream::StreamExt;

use crate::datasource::repository::episode::models::WithResolutions;
use crate::models as domain_models;

pub mod models {
    use anyhow::anyhow;
    use chrono::{DateTime, Utc};
    use sqlx::types::Uuid;

    use crate::errors::InternalError;
    use crate::models as domain_models;

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

    impl TryFrom<Download> for domain_models::Download {
        type Error = InternalError;

        fn try_from(a: Download) -> Result<Self, Self::Error> {
            Ok(Self {
                comments: a
                    .comments
                    .ok_or(anyhow!("required Download.comments was None"))?,
                resolution: a.resolution,
                torrent: a.torrent,
                file_name: a.file_name,
                published_date: a.created_at,
            })
        }
    }
}

pub async fn insert(
    pool: Pool<Postgres>,
    episode_id: &Uuid,
    download: &domain_models::Download,
) -> Result<()> {
    sqlx::query_file!(
        "queries/insert_episode_download_resolution.sql",
        episode_id,
        download.resolution,
        download.torrent,
        Some(&download.file_name),
        download.comments,
        Option::<String>::None,
        download.published_date,
    )
    .execute(&pool)
    .await?;
    Ok(())
}

pub(super) async fn get_data_downloads(
    pool: Pool<Postgres>,
    map: &mut HashMap<Uuid, WithResolutions>,
) -> Result<()> {
    let mut qb = QueryBuilder::new("SELECT * FROM episode_download_resolution");
    qb.push(" WHERE episode_download_id in (");
    let mut separated = qb.separated(", ");
    for &id in map.keys() {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    qb.push(
        " ORDER BY array_position(array['2160p', '1080p', '720p', '540p', '480p'], resolution)",
    );
    let query = qb.build_query_as::<models::Download>();
    let mut stream = query.fetch(&pool);
    while let Some(row) = stream.next().await {
        let row = row?;
        if let Some(group) = map.get_mut(&row.episode_download_id) {
            group.resolutions.push(row);
        }
    }
    Ok(())
}
