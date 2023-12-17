use std::collections::HashMap;

use ahash::RandomState;
use anyhow::Result;
use futures::stream::BoxStream;
use futures::StreamExt;
use sqlx::types::Uuid;
use sqlx::{Executor, Postgres, QueryBuilder};

use crate::datasource::repository::download::models::DownloadEntity;
use crate::models::Download;

pub(super) mod models {
    use chrono::{DateTime, Utc};
    use sqlx::types::Uuid;

    use crate::models::Download;

    #[derive(Debug, sqlx::FromRow)]
    pub(super) struct DownloadEntity {
        pub id: Uuid,
        pub resolution: i16,
        pub torrent: String,
        pub file_name: String,
        pub comments: String,
        #[sqlx(rename = "magnet")]
        pub _magnet: Option<String>,
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

// TODO replace repeated variant functions with generics
pub(super) async fn insert_batch<'e, E>(executor: E, id: Uuid, download: &Download) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_file!(
        "queries/batch/insert_batch_download_resolution.sql",
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

pub(super) async fn insert_episode<'e, E>(executor: E, id: Uuid, download: &Download) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_file!(
        "queries/episode/insert_episode_download_resolution.sql",
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

pub(super) async fn insert_movie<'e, E>(executor: E, id: Uuid, download: &Download) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_file!(
        "queries/movie/insert_movie_download_resolution.sql",
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

pub(super) async fn get_for_batches<'e, E>(
    executor: E,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<Download>, RandomState>>
where
    E: Executor<'e, Database = Postgres>,
{
    get_results_for_variant("batch", executor, ids).await
}

pub(super) async fn get_for_episodes<'e, E>(
    executor: E,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<Download>, RandomState>>
where
    E: Executor<'e, Database = Postgres>,
{
    get_results_for_variant("episode", executor, ids).await
}

pub(super) async fn get_for_movies<'e, E>(
    executor: E,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<Download>, RandomState>>
where
    E: Executor<'e, Database = Postgres>,
{
    get_results_for_variant("movie", executor, ids).await
}

async fn get_results_for_variant<'e, E>(
    variant: &'static str,
    executor: E,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<Download>, RandomState>>
where
    E: Executor<'e, Database = Postgres>,
{
    let mut qb = QueryBuilder::new(format!(
        "SELECT {variant}_download_id as id, *
        FROM {variant}_download_resolution
        WHERE {variant}_download_id in ("
    ));
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    qb.push(") ORDER BY resolution DESC");

    let query = qb.build_query_as::<DownloadEntity>();
    get_results(query.fetch(executor)).await
}

async fn get_results(
    mut stream: BoxStream<'_, Result<DownloadEntity, sqlx::Error>>,
) -> Result<HashMap<Uuid, Vec<Download>, RandomState>> {
    let mut episodes = HashMap::<Uuid, Vec<Download>, RandomState>::default();
    while let Some(row) = stream.next().await {
        let download_entity = row?;
        let id = download_entity.id;
        let download = download_entity.try_into()?;
        episodes.entry(id).or_default().push(download)
    }
    Ok(episodes)
}
