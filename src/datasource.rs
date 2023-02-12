pub mod models {
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
}

pub mod repositories {
    pub mod downloads {
        use crate::datasource::models as data_models;
        use crate::datasource::models::EpisodeWithResolutions;
        use crate::{models, DBPool};
        use anyhow::Result;
        use sqlx::types::Uuid;
        use sqlx::QueryBuilder;
        use std::cmp::Reverse;
        use std::collections::hash_map::RandomState;
        use std::collections::HashMap;
        use tokio_stream::StreamExt;

        #[derive(Debug, Default)]
        pub struct EpisodeQueryOptions {
            pub title: Option<String>,
        }

        pub async fn get_episodes(
            pool: &DBPool,
            options: Option<EpisodeQueryOptions>,
        ) -> Result<Vec<models::Episode>> {
            let rows = get_data_episodes(pool, options).await?;
            let episodes = rows
                .into_iter()
                .map(|v| v.try_into())
                .collect::<Result<Vec<models::Episode>, _>>()?;
            Ok(episodes)
        }

        pub async fn get_episode_with_downloads(
            pool: &DBPool,
            options: Option<EpisodeQueryOptions>,
        ) -> Result<Vec<models::DownloadGroup>> {
            let rows = get_data_episodes(pool, options).await?;
            let iter = rows.into_iter().map(|r| {
                let id = r.id;
                let group = EpisodeWithResolutions {
                    episode: r,
                    resolutions: vec![],
                };
                (id, group)
            });
            let mut map: HashMap<Uuid, EpisodeWithResolutions, RandomState> =
                HashMap::from_iter(iter);

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
            let query = qb.build_query_as::<data_models::Download>();
            let mut stream = query.fetch(pool);
            while let Some(row) = stream.next().await {
                let row = row?;
                if let Some(group) = map.get_mut(&row.episode_download_id) {
                    group.resolutions.push(row);
                }
            }
            let mut episodes = map
                .into_values()
                .map(|v| v.try_into())
                .collect::<Result<Vec<models::DownloadGroup>, _>>()?;
            episodes.sort_by_key(|ep| Reverse(ep.episode.published_date));
            Ok(episodes)
        }

        async fn get_data_episodes(
            pool: &DBPool,
            options: Option<EpisodeQueryOptions>,
        ) -> Result<Vec<data_models::Episode>> {
            let mut qb = QueryBuilder::new("SELECT * FROM episode_download");
            let mut has_where = false;
            if let Some(options) = options {
                if let Some(title) = options.title {
                    qb.push(if has_where { " AND" } else { " WHERE" })
                        .push(" title ILIKE ")
                        .push_bind(title);
                    has_where = true;
                }
            }
            let query = qb
                .push(" ORDER BY created_at DESC")
                .push(" LIMIT 25")
                .build_query_as::<data_models::Episode>();
            let rows = query.fetch_all(pool).await?;
            Ok(rows)
        }
    }
}
