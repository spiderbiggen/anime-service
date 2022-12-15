use crate::{models, Error, HyperClient};
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::log::{error, warn};

#[derive(Debug, Clone)]
pub(crate) struct Poll {
    client: HyperClient,
    database: Pool<Postgres>,
    interval: Duration,
}

const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);

impl Poll {
    pub fn new(client: HyperClient, database: Pool<Postgres>) -> Self {
        Self {
            client,
            database,
            interval: DEFAULT_INTERVAL,
        }
    }

    pub async fn run(&self) {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            interval.tick().await;
            self.execute().await;
        }
    }

    async fn execute(&self) {
        let groups = match self.get_groups().await {
            Ok(groups) => groups,
            Err(err) => {
                error!("Failed to get anime: {:?}", err);
                return;
            }
        };
        for group in groups {
            let insert = sqlx::query_file!(
                "queries/insert_episode_download.sql",
                group.episode.title,
                group.episode.episode.map(|e| e as i32),
                group.episode.decimal.map(|e| e as i32),
                group.episode.version.map(|e| e as i32),
                group.episode.pub_date,
            )
            .fetch_one(&self.database)
            .await;
            let anime_id: sqlx::types::Uuid = match insert {
                Ok(r) => r.id,
                Err(err) => {
                    warn!("Failed to save anime: {:?}", err);
                    continue;
                }
            };
            for download in group.downloads {
                let insert = sqlx::query_file!(
                    "queries/insert_episode_download_resolution.sql",
                    anime_id,
                    download.resolution,
                    download.torrent,
                    Some(download.file_name),
                    download.comments,
                    Option::<String>::None,
                    download.pub_date,
                )
                .execute(&self.database)
                .await;
                if let Err(err) = insert {
                    warn!("Failed to save anime: {:?}", err)
                }
            }
        }
    }

    async fn get_groups(&self) -> Result<Vec<models::DownloadGroup>, Error> {
        let result = nyaa::groups(self.client.clone(), "")
            .await?
            .into_iter()
            .map(|e| e.into())
            .collect();
        Ok(result)
    }
}
