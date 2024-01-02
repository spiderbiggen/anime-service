use std::collections::HashMap;

use ahash::RandomState;
use anyhow::Result;
use futures::StreamExt;
use sqlx::types::Uuid;
use sqlx::{query_file_as, Executor, Postgres};

use crate::datasource::repository::download_resolutions::models::DownloadEntity;
use crate::models::Download;

pub(super) mod models {
    use chrono::{DateTime, Utc};
    use sqlx::types::Uuid;

    use crate::models::Download;

    #[derive(Debug, sqlx::FromRow)]
    pub(super) struct DownloadEntity {
        pub download_id: Uuid,
        pub resolution: i16,
        pub torrent: String,
        pub file_name: String,
        pub comments: String,
        #[allow(dead_code)]
        pub magnet: Option<String>,
        pub created_at: DateTime<Utc>,
    }

    impl From<DownloadEntity> for Download {
        fn from(value: DownloadEntity) -> Self {
            Self {
                comments: value.comments,
                resolution: value.resolution as u16,
                torrent: value.torrent,
                file_name: value.file_name,
                published_date: value.created_at,
            }
        }
    }
}

pub(super) async fn insert<'e, E>(executor: E, id: Uuid, download: &Download) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_file!(
        "queries/insert_download_resolution.sql",
        id,
        download.resolution as i16,
        download.torrent,
        &download.file_name,
        download.comments,
        Option::<String>::None,
        download.published_date,
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub(super) async fn resolutions_for_downloads<'e, E>(
    executor: E,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<Download>, RandomState>>
where
    E: Executor<'e, Database = Postgres>,
{
    let query = query_file_as!(
        DownloadEntity,
        "queries/query_download_resolution_by_ids.sql",
        ids,
    );
    let mut stream = query.fetch(executor);
    let mut episodes = HashMap::<Uuid, Vec<Download>, RandomState>::default();
    while let Some(row) = stream.next().await {
        let download_entity = row?;
        let id = download_entity.download_id;
        episodes.entry(id).or_default().push(download_entity.into())
    }
    Ok(episodes)
}
