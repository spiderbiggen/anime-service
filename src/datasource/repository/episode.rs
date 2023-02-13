use std::cmp::Reverse;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;

use anyhow::Result;
use sqlx::types::Uuid;
use sqlx::{Pool, Postgres, QueryBuilder, Transaction};

use crate::datasource::repository::download;
use crate::models as domain_models;

pub mod models {
    use chrono::{DateTime, Utc};
    use sqlx::types::Uuid;

    pub(super) use crate::datasource::repository::download::models::Download;
    use crate::errors::InternalError;
    use crate::models as domain_models;

    #[derive(Debug, sqlx::FromRow)]
    pub struct Episode {
        pub id: Uuid,
        pub title: String,
        pub episode: Option<i32>,
        pub decimal: Option<i32>,
        pub version: Option<i32>,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    impl TryFrom<Episode> for domain_models::Episode {
        type Error = InternalError;

        fn try_from(a: Episode) -> Result<Self, Self::Error> {
            let ep = match a.episode {
                Some(i) => Some(i.try_into()?),
                None => None,
            };
            let dec = match a.decimal {
                Some(i) => Some(i.try_into()?),
                None => None,
            };
            let ver = match a.version {
                Some(i) => Some(i.try_into()?),
                None => None,
            };
            Ok(Self {
                title: a.title,
                episode: ep,
                decimal: dec,
                version: ver,
                created_at: a.created_at,
                updated_at: a.updated_at,
            })
        }
    }

    #[derive(Debug, sqlx::FromRow)]
    pub struct UpsertResult {
        pub id: Uuid,
        pub resolutions: Option<Vec<String>>,
    }

    #[derive(Debug, sqlx::FromRow)]
    pub struct WithResolutions {
        pub episode: Episode,
        pub resolutions: Vec<Download>,
    }

    impl TryFrom<WithResolutions> for domain_models::DownloadGroup {
        type Error = InternalError;

        fn try_from(a: WithResolutions) -> Result<Self, Self::Error> {
            Ok(Self {
                episode: a.episode.try_into()?,
                downloads: a
                    .resolutions
                    .into_iter()
                    .map(|it| it.try_into())
                    .collect::<Result<_, _>>()?,
            })
        }
    }
}

#[derive(Debug, Default)]
pub struct EpisodeQueryOptions {
    pub title: Option<String>,
}

pub async fn upsert(
    pool: Pool<Postgres>,
    episode: &domain_models::Episode,
) -> Result<models::UpsertResult> {
    let mut tx = pool.begin().await?;
    if let Some(record) = get_episode_by_unique_fields(&mut tx, episode).await? {
        return Ok(record);
    }

    let result = models::UpsertResult {
        id: insert_episode(&mut tx, episode).await?,
        resolutions: None,
    };
    tx.commit().await?;
    Ok(result)
}

async fn get_episode_by_unique_fields(
    pool: &mut Transaction<'_, Postgres>,
    episode: &domain_models::Episode,
) -> Result<Option<models::UpsertResult>> {
    let result = sqlx::query_file_as!(
        models::UpsertResult,
        "queries/query_episode_download_by_unique.sql",
        Option::<String>::None,
        episode.title,
        episode.episode.map(|e| e as i32),
        episode.decimal.map(|e| e as i32),
        episode.version.map(|e| e as i32)
    )
    .fetch_optional(pool)
    .await?;
    Ok(result)
}

async fn insert_episode(
    pool: &mut Transaction<'_, Postgres>,
    episode: &domain_models::Episode,
) -> Result<Uuid> {
    let result = sqlx::query_file!(
        "queries/insert_episode_download.sql",
        episode.title,
        episode.episode.map(|e| e as i32),
        episode.decimal.map(|e| e as i32),
        episode.version.map(|e| e as i32),
        episode.created_at,
        episode.updated_at,
    )
    .fetch_one(pool)
    .await?
    .id;
    Ok(result)
}

pub async fn get_collection(
    pool: Pool<Postgres>,
    options: Option<EpisodeQueryOptions>,
) -> Result<Vec<domain_models::Episode>> {
    let rows = get_data_episodes(pool, options).await?;
    let episodes = rows
        .into_iter()
        .map(|v| v.try_into())
        .collect::<Result<Vec<domain_models::Episode>, _>>()?;
    Ok(episodes)
}

pub async fn get_with_downloads(
    pool: Pool<Postgres>,
    options: Option<EpisodeQueryOptions>,
) -> Result<Vec<domain_models::DownloadGroup>> {
    let rows = get_data_episodes(pool.clone(), options).await?;
    let iter = rows.into_iter().map(|r| {
        let id = r.id;
        let group = models::WithResolutions {
            episode: r,
            resolutions: vec![],
        };
        (id, group)
    });
    let mut map: HashMap<Uuid, models::WithResolutions, RandomState> = HashMap::from_iter(iter);

    download::get_data_downloads(pool.clone(), &mut map).await?;
    let mut episodes = map
        .into_values()
        .map(|v| v.try_into())
        .collect::<Result<Vec<domain_models::DownloadGroup>, _>>()?;
    episodes.sort_by_key(|ep| Reverse(ep.episode.updated_at));
    Ok(episodes)
}

async fn get_data_episodes(
    pool: Pool<Postgres>,
    options: Option<EpisodeQueryOptions>,
) -> Result<Vec<models::Episode>> {
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
        .build_query_as::<models::Episode>();
    let rows = query.fetch_all(&pool).await?;
    Ok(rows)
}
