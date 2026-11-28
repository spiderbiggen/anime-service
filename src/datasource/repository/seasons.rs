use std::collections::HashMap;

use ahash::RandomState;
use anyhow::Result;
use sqlx::types::Json;
use sqlx::{Executor, Pool, Postgres, query_file};

use crate::models::{DownloadGroup, TmdbMatch};
use crate::seasons::{self, CachedSeason, SeasonCount, SeasonOverride, Target};

/// What is known about the show of a release title.
#[derive(Debug)]
pub(crate) struct TitleMapping {
    pub tmdb_id: u32,
    pub season_override: Option<SeasonOverride>,
    /// Empty until the show's seasons have been fetched.
    pub counts: Vec<SeasonCount>,
    /// Whether the show was fetched more than a day ago.
    pub show_is_old: bool,
}

/// A cached season and whether it was fetched more than a day ago.
#[derive(Debug)]
pub(crate) struct StoredSeason {
    pub season: CachedSeason,
    pub is_old: bool,
}

/// Everything cached that is needed to match the releases of some titles.
#[derive(Debug, Default)]
pub(crate) struct SeasonData {
    pub titles: HashMap<String, TitleMapping, RandomState>,
    /// Keyed by show id and season number.
    pub seasons: HashMap<(u32, u32), StoredSeason, RandomState>,
}

impl SeasonData {
    /// Returns where a release would be on TMDB, along with the id of its show.
    pub(crate) fn target(&self, group: &DownloadGroup) -> Option<(u32, Target)> {
        let mapping = self.titles.get(&group.title)?;
        let target = seasons::locate(
            &group.title,
            &group.variant,
            mapping.season_override,
            &mapping.counts,
        )?;
        Some((mapping.tmdb_id, target))
    }

    /// Matches a release against the cached data.
    pub(crate) fn find(&self, group: &DownloadGroup) -> Option<TmdbMatch> {
        let (tmdb_id, target) = self.target(group)?;
        let stored = self.seasons.get(&(tmdb_id, target.season))?;
        let overridden = self.titles.get(&group.title)?.season_override.is_some();
        seasons::verify(
            target,
            &group.variant,
            group.created_at,
            overridden,
            &stored.season,
        )
    }
}

/// Loads the cached season data of the given titles.
pub(crate) async fn data_for_titles(
    pool: &Pool<Postgres>,
    titles: &[String],
) -> Result<SeasonData> {
    let mappings = query_file!("queries/seasons/mappings_for_titles.sql", titles)
        .fetch_all(pool)
        .await?;
    let seasons = query_file!("queries/seasons/seasons_for_titles.sql", titles)
        .fetch_all(pool)
        .await?;
    Ok(SeasonData {
        titles: mappings
            .into_iter()
            .map(|row| {
                let season_override = row.season_number.zip(row.episode_offset).and_then(
                    |(season, episode_offset)| {
                        Some(SeasonOverride {
                            season: u32::try_from(season).ok()?,
                            episode_offset,
                        })
                    },
                );
                let mapping = TitleMapping {
                    tmdb_id: row.tmdb_id.cast_unsigned(),
                    season_override,
                    counts: row.seasons.map_or_default(|seasons| seasons.0),
                    show_is_old: row.show_is_old,
                };
                (row.title, mapping)
            })
            .collect(),
        seasons: seasons
            .into_iter()
            .map(|row| {
                let key = (
                    row.tmdb_id.cast_unsigned(),
                    row.season_number.cast_unsigned(),
                );
                let stored = StoredSeason {
                    season: row.data.0,
                    is_old: row.is_old,
                };
                (key, stored)
            })
            .collect(),
    })
}

pub(crate) async fn upsert_season<'e, E>(
    executor: E,
    tmdb_id: u32,
    season: &CachedSeason,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    query_file!(
        "queries/seasons/upsert_season.sql",
        tmdb_id.cast_signed(),
        season.number.cast_signed(),
        Json(season) as _
    )
    .execute(executor)
    .await?;
    Ok(())
}
