use std::collections::HashMap;

use ahash::RandomState;
use anyhow::Result;
use sqlx::types::Json;
use sqlx::{Executor, Postgres, query_file};

use crate::models::ShowSummary;
use crate::seasons::SeasonCount;

/// Loads the show summary of every title that has a matched, already fetched show.
pub(crate) async fn summaries_for_titles<'e, E>(
    executor: E,
    titles: &[String],
) -> Result<HashMap<String, ShowSummary, RandomState>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = query_file!("queries/shows/summaries_for_titles.sql", titles)
        .fetch_all(executor)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.title, row.data.0))
        .collect())
}

/// Returns the titles that still need a lookup: never looked up, or looked up without a result
/// more than a week ago. Titles with a manual match are never returned.
pub(crate) async fn unresolved_titles<'e, E>(executor: E, titles: &[String]) -> Result<Vec<String>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = query_file!("queries/shows/unresolved_titles.sql", titles)
        .fetch_all(executor)
        .await?;
    Ok(rows.into_iter().map(|row| row.title).collect())
}

/// Returns the matched show ids whose summary is missing or older than 30 days.
pub(crate) async fn show_ids_to_refresh<'e, E>(executor: E, titles: &[String]) -> Result<Vec<u32>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = query_file!("queries/shows/show_ids_to_refresh.sql", titles)
        .fetch_all(executor)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.tmdb_id.cast_unsigned())
        .collect())
}

/// Records the result of an automatic lookup. A manual match is never overwritten.
pub(crate) async fn upsert_title_match<'e, E>(
    executor: E,
    title: &str,
    tmdb_id: Option<u32>,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    query_file!(
        "queries/shows/upsert_title_match.sql",
        title,
        tmdb_id.map(u32::cast_signed)
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub(crate) async fn upsert_show<'e, E>(
    executor: E,
    tmdb_id: u32,
    summary: &ShowSummary,
    seasons: &[SeasonCount],
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    query_file!(
        "queries/shows/upsert_show.sql",
        tmdb_id.cast_signed(),
        Json(summary) as _,
        Json(seasons) as _
    )
    .execute(executor)
    .await?;
    Ok(())
}
