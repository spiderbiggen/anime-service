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
    ///
    /// A title with a season suffix that finds nothing is searched again without it. The result is
    /// always recorded under the full title, which is what downloads are joined on.
    async fn match_title(&self, title: &str) -> anyhow::Result<()> {
        let mut found = self.search_show(title).await?;
        if found.is_none()
            && let Some(base_title) = without_season_suffix(title)
        {
            found = self.search_show(base_title).await?;
        }
        debug!(title, ?found, "matched a title");
        shows::upsert_title_match(&self.pool, title, found).await?;
        Ok(())
    }

    /// Returns the id of the show that best matches a search query.
    async fn search_show(&self, query: &str) -> anyhow::Result<Option<u32>> {
        let page = tmdb::search::tv(&self.tmdb, &TvSearchParams::new(query)).await?;
        Ok(select_match(&page.results).map(|result| result.id))
    }

    /// Fetches the details of a show and stores its summary.
    async fn refresh_show(&self, id: u32) -> anyhow::Result<()> {
        let (details, external_ids, alternative_titles) = tokio::join!(
            tmdb::tv::details(&self.tmdb, id, None),
            tmdb::tv::external_ids(&self.tmdb, id),
            tmdb::tv::alternative_titles(&self.tmdb, id)
        );
        let details = details?;
        // The TVDB id and the romaji name are nice-to-haves, so a failure here must not discard
        // the show.
        let external_ids = external_ids
            .inspect_err(|error| warn!(%error, id, "failed to fetch external ids"))
            .ok();
        let alternative_titles = alternative_titles
            .inspect_err(|error| warn!(%error, id, "failed to fetch alternative titles"))
            .unwrap_or_default();
        let summary = ShowSummary::new(details, external_ids, &alternative_titles);
        shows::upsert_show(&self.pool, id, &summary).await?;
        Ok(())
    }
}

/// Picks the first result that is tagged as animation. TMDB search matches alternative titles, so
/// the top animation result is usually the right show for a romanized release title.
fn select_match(results: &[TvSearchResult]) -> Option<&TvSearchResult> {
    results.iter().find(|result| result.is_animation())
}

/// Strips a trailing season marker such as ` S2`, which release titles carry but TMDB show names
/// usually don't. Returns `None` when there is no such marker.
fn without_season_suffix(title: &str) -> Option<&str> {
    let (base, suffix) = title.rsplit_once(' ')?;
    let season = suffix.strip_prefix('S')?;
    let is_season = !season.is_empty() && season.bytes().all(|byte| byte.is_ascii_digit());
    (is_season && !base.is_empty()).then_some(base)
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
    fn strips_a_trailing_season_suffix() {
        assert_eq!(
            without_season_suffix("Tensei Shitara Ken Deshita S2"),
            Some("Tensei Shitara Ken Deshita")
        );
        assert_eq!(without_season_suffix("Sample S12"), Some("Sample"));
    }

    #[test]
    fn titles_without_a_season_suffix_are_not_stripped() {
        for title in [
            "Sample",
            "Sample S",
            "Sample 2",
            "Sample S2 Part",
            "Sample SS2",
            "Sample S2x",
            "S2",
            " S2",
        ] {
            assert_eq!(without_season_suffix(title), None, "{title:?}");
        }
    }

    #[test]
    fn no_animation_result_means_no_match() {
        let results = results(r#"[{ "id": 1, "name": "Drama", "genre_ids": [18] }]"#);

        assert!(select_match(&results).is_none());
        assert!(select_match(&[]).is_none());
    }
}
