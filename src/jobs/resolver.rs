//! Resolves release titles to TMDB shows, so that downloads can carry a banner and a synopsis.
//!
//! Resolution is best-effort: it must never stop a release from being delivered, so every failure
//! is logged and the download is simply sent without a show.

use std::collections::BTreeSet;
use std::time::Duration;

use futures::StreamExt;
use futures::stream;
use tmdb::TmdbClient;
use tmdb::search::{TvSearchParams, TvSearchResult};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::datasource::repository::shows;
use crate::models::{DownloadGroup, ShowSummary};
use crate::state::DBPool;

/// How long a notification may be delayed by looking shows up.
const RESOLVE_BUDGET: Duration = Duration::from_secs(5);
/// How many TMDB lookups run at once.
const CONCURRENCY: usize = 4;

#[derive(Debug, Clone)]
pub struct ShowResolver {
    tmdb: TmdbClient,
    pool: DBPool,
}

impl ShowResolver {
    #[must_use]
    pub fn new(tmdb: TmdbClient, pool: DBPool) -> Self {
        Self { tmdb, pool }
    }

    /// Looks up the shows of any titles that are not matched yet (within [`RESOLVE_BUDGET`]),
    /// then attaches the known show to each group. Never fails: problems are logged.
    pub async fn attach(&self, groups: &mut [DownloadGroup]) {
        let titles: Vec<String> = groups
            .iter()
            .map(|group| group.title.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if titles.is_empty() {
            return;
        }

        if timeout(RESOLVE_BUDGET, self.resolve(&titles))
            .await
            .is_err()
        {
            warn!("resolving shows took longer than {RESOLVE_BUDGET:?}, continuing without them");
        }

        match shows::summaries_for_titles(&self.pool, &titles).await {
            Ok(summaries) => {
                for group in groups {
                    group.show = summaries.get(&group.title).cloned();
                }
            }
            Err(error) => warn!(%error, "failed to load shows for downloads"),
        }
    }

    async fn resolve(&self, titles: &[String]) {
        if let Err(error) = self.try_resolve(titles).await {
            warn!(%error, "failed to resolve shows");
        }
    }

    async fn try_resolve(&self, titles: &[String]) -> anyhow::Result<()> {
        let unresolved = shows::unresolved_titles(&self.pool, titles).await?;
        stream::iter(unresolved)
            .for_each_concurrent(CONCURRENCY, |title| async move {
                if let Err(error) = self.match_title(&title).await {
                    warn!(%error, title, "failed to match a title to a show");
                }
            })
            .await;

        let stale = shows::show_ids_to_refresh(&self.pool, titles).await?;
        stream::iter(stale)
            .for_each_concurrent(CONCURRENCY, |id| async move {
                if let Err(error) = self.refresh_show(id).await {
                    warn!(%error, id, "failed to refresh a show");
                }
            })
            .await;
        Ok(())
    }

    /// Searches TMDB for a title and records the best match, or that there is none.
    async fn match_title(&self, title: &str) -> anyhow::Result<()> {
        let page = tmdb::search::tv(&self.tmdb, &TvSearchParams::new(title)).await?;
        let found = select_match(&page.results).map(|result| result.id);
        debug!(title, ?found, "matched a title");
        shows::upsert_title_match(&self.pool, title, found).await?;
        Ok(())
    }

    /// Fetches the details of a show and stores its summary.
    async fn refresh_show(&self, id: u32) -> anyhow::Result<()> {
        let (details, external_ids) = tokio::join!(
            tmdb::tv::details(&self.tmdb, id, None),
            tmdb::tv::external_ids(&self.tmdb, id)
        );
        let details = details?;
        // The TVDB id is a nice-to-have, so a failure here must not discard the show.
        let external_ids = external_ids
            .inspect_err(|error| warn!(%error, id, "failed to fetch external ids"))
            .ok();
        shows::upsert_show(&self.pool, id, &ShowSummary::new(details, external_ids)).await?;
        Ok(())
    }
}

/// Picks the first result that is tagged as animation. TMDB search matches alternative titles, so
/// the top animation result is usually the right show for a romanized release title.
fn select_match(results: &[TvSearchResult]) -> Option<&TvSearchResult> {
    results.iter().find(|result| result.is_animation())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results(json: &str) -> Vec<TvSearchResult> {
        serde_json::from_str(json).expect("valid results")
    }

    #[test]
    fn picks_the_first_animation_result() {
        let results = results(
            r#"[
                { "id": 1, "name": "Live action", "genre_ids": [18] },
                { "id": 2, "name": "Anime", "genre_ids": [16, 18] },
                { "id": 3, "name": "Other anime", "genre_ids": [16] }
            ]"#,
        );

        assert_eq!(select_match(&results).map(|r| r.id), Some(2));
    }

    #[test]
    fn no_animation_result_means_no_match() {
        let results = results(r#"[{ "id": 1, "name": "Drama", "genre_ids": [18] }]"#);

        assert!(select_match(&results).is_none());
        assert!(select_match(&[]).is_none());
    }
}
