//! Resolves releases to TMDB shows and seasons, so that downloads can carry a banner, a synopsis
//! and TMDB's season and episode numbers.
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

use crate::datasource::repository::seasons::{self, SeasonData};
use crate::datasource::repository::shows;
use crate::models::{DownloadGroup, ShowSummary};
use crate::seasons::{CachedEpisode, CachedSeason, SeasonCount, Target, release_range};
use crate::state::DBPool;

/// How long a notification may be delayed by looking shows up.
const RESOLVE_BUDGET: Duration = Duration::from_secs(15);
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

    /// Looks up whatever is not cached yet about the shows and seasons of the groups (within
    /// [`RESOLVE_BUDGET`]), then attaches what is known to each group. Never fails: problems are
    /// logged.
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

        if timeout(RESOLVE_BUDGET, self.resolve(&titles, groups))
            .await
            .is_err()
        {
            warn!("resolving shows took longer than {RESOLVE_BUDGET:?}, continuing without them");
        }

        match shows::summaries_for_titles(&self.pool, &titles).await {
            Ok(summaries) => {
                for group in groups.iter_mut() {
                    group.show = summaries.get(&group.title).cloned();
                }
            }
            Err(error) => warn!(%error, "failed to load shows for downloads"),
        }

        match seasons::data_for_titles(&self.pool, &titles).await {
            Ok(data) => {
                for group in groups {
                    group.tmdb = data.find(group);
                }
            }
            Err(error) => warn!(%error, "failed to load seasons for downloads"),
        }
    }

    async fn resolve(&self, titles: &[String], groups: &[DownloadGroup]) {
        if let Err(error) = self.try_resolve_shows(titles).await {
            warn!(%error, "failed to resolve shows");
        }
        if let Err(error) = self.try_resolve_seasons(titles, groups).await {
            warn!(%error, "failed to resolve seasons");
        }
    }

    async fn try_resolve_shows(&self, titles: &[String]) -> anyhow::Result<()> {
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

    /// Fetches the seasons that the releases need and that are not cached well enough.
    async fn try_resolve_seasons(
        &self,
        titles: &[String],
        groups: &[DownloadGroup],
    ) -> anyhow::Result<()> {
        let mut data = seasons::data_for_titles(&self.pool, titles).await?;

        // A release that does not fit the known seasons may belong to one TMDB added since.
        let outdated = shows_with_unknown_seasons(&data, groups);
        if !outdated.is_empty() {
            stream::iter(outdated)
                .for_each_concurrent(CONCURRENCY, |id| async move {
                    if let Err(error) = self.refresh_show(id).await {
                        warn!(%error, id, "failed to refresh a show");
                    }
                })
                .await;
            data = seasons::data_for_titles(&self.pool, titles).await?;
        }

        stream::iter(seasons_to_fetch(&data, groups))
            .for_each_concurrent(CONCURRENCY, |(id, number)| async move {
                if let Err(error) = self.refresh_season(id, number).await {
                    warn!(%error, id, number, "failed to fetch a season");
                }
            })
            .await;
        Ok(())
    }

    /// Fetches a season of a show and caches it.
    async fn refresh_season(&self, id: u32, number: u32) -> anyhow::Result<()> {
        let season = tmdb::tv::season(&self.tmdb, id, number).await?;
        seasons::upsert_season(&self.pool, id, &CachedSeason::from(season)).await?;
        Ok(())
    }

    /// Searches TMDB for a title and records the best match, or that there is none.
    ///
    /// A title with a season suffix that finds nothing is searched again without it. The result is
    /// always recorded under the full title, which is what downloads are joined on.
    async fn match_title(&self, title: &str) -> anyhow::Result<()> {
        let mut found = self.search_show(title).await?;
        if found.is_none()
            && let Some((base_title, _)) = split_season_suffix(title)
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
        let counts: Vec<SeasonCount> = details.seasons.iter().map(Into::into).collect();
        let summary = ShowSummary::new(details, external_ids, &alternative_titles);
        shows::upsert_show(&self.pool, id, &summary, &counts).await?;
        Ok(())
    }
}

/// Returns whether a show's season list includes the season of a target.
fn is_listed(counts: &[SeasonCount], target: Target) -> bool {
    counts.iter().any(|count| count.number == target.season)
}

/// Returns the shows, not fetched in the last day, whose season list has no place for one of the
/// releases.
fn shows_with_unknown_seasons(data: &SeasonData, groups: &[DownloadGroup]) -> BTreeSet<u32> {
    groups
        .iter()
        .filter(|group| release_range(&group.variant).is_some())
        .filter_map(|group| {
            let mapping = data.titles.get(&group.title)?;
            let listed = data
                .target(group)
                .is_some_and(|(_, target)| is_listed(&mapping.counts, target));
            (mapping.show_is_old && !listed).then_some(mapping.tmdb_id)
        })
        .collect()
}

/// Returns the seasons, as show id and season number, to fetch for the releases: those that are
/// not cached, were cached more than a day ago, or lack one of the episodes or its title.
fn seasons_to_fetch(data: &SeasonData, groups: &[DownloadGroup]) -> BTreeSet<(u32, u32)> {
    groups
        .iter()
        .filter_map(|group| {
            let (id, target) = data.target(group)?;
            let counts = &data.titles.get(&group.title)?.counts;
            // Asking for a season the show does not list would only fail.
            if !counts.is_empty() && !is_listed(counts, target) {
                return None;
            }
            let is_good_enough = data
                .seasons
                .get(&(id, target.season))
                .is_some_and(|stored| {
                    !stored.is_old
                        && [target.first, target.last].into_iter().all(|number| {
                            stored
                                .season
                                .episode(number)
                                .is_some_and(CachedEpisode::has_title)
                        })
                });
            (!is_good_enough).then_some((id, target.season))
        })
        .collect()
}

/// Picks the first result that is tagged as animation. TMDB search matches alternative titles, so
/// the top animation result is usually the right show for a romanized release title.
fn select_match(results: &[TvSearchResult]) -> Option<&TvSearchResult> {
    results.iter().find(|result| result.is_animation())
}

/// Splits a trailing season marker such as ` S2`, which release titles carry but TMDB show names
/// usually don't, into the rest of the title and the season number. Returns `None` when there is
/// no such marker.
pub(crate) fn split_season_suffix(title: &str) -> Option<(&str, u32)> {
    let (base, suffix) = title.rsplit_once(' ')?;
    let digits = suffix.strip_prefix('S')?;
    if base.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((base, digits.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use super::*;
    use crate::datasource::repository::seasons::{StoredSeason, TitleMapping};
    use crate::models::{DownloadVariant, Episode};

    fn release(title: &str, episode: u32) -> DownloadGroup {
        DownloadGroup {
            title: title.to_owned(),
            variant: DownloadVariant::Episode(Episode {
                episode,
                decimal: None,
                version: None,
                extra: None,
            }),
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: Timestamp::UNIX_EPOCH,
            downloads: Vec::new(),
            show: None,
            tmdb: None,
        }
    }

    /// Show 7, matched to `title`, with a single season of 12 episodes.
    fn season_data(title: &str, show_is_old: bool) -> SeasonData {
        let mut data = SeasonData::default();
        data.titles.insert(
            title.to_owned(),
            TitleMapping {
                tmdb_id: 7,
                season_override: None,
                counts: vec![SeasonCount {
                    number: 1,
                    episode_count: 12,
                }],
                show_is_old,
            },
        );
        data
    }

    fn cache(data: &mut SeasonData, episode_name: &str, is_old: bool) {
        let season = CachedSeason {
            number: 1,
            overview: String::new(),
            poster_path: None,
            episodes: vec![CachedEpisode {
                number: 3,
                name: episode_name.to_owned(),
                air_date: None,
            }],
        };
        data.seasons.insert((7, 1), StoredSeason { season, is_old });
    }

    #[test]
    fn a_season_is_fetched_when_the_cache_is_not_good_enough() {
        let releases = [release("Show", 3)];
        let wanted = |data: &SeasonData| !seasons_to_fetch(data, &releases).is_empty();
        let mut data = season_data("Show", false);
        assert!(wanted(&data), "not cached");

        cache(&mut data, "A title", false);
        assert!(!wanted(&data), "fresh and complete");
        assert!(
            !seasons_to_fetch(&data, &[release("Show", 4)]).is_empty(),
            "episode missing"
        );

        cache(&mut data, "A title", true);
        assert!(wanted(&data), "old");

        cache(&mut data, "Episode 3", false);
        assert!(wanted(&data), "placeholder title");
    }

    #[test]
    fn unlisted_seasons_are_not_fetched_but_refresh_an_old_show() {
        let releases = [release("Show S2", 1)];
        let old = season_data("Show S2", true);
        let recent = season_data("Show S2", false);

        assert!(seasons_to_fetch(&old, &releases).is_empty());
        assert_eq!(
            shows_with_unknown_seasons(&old, &releases),
            BTreeSet::from([7])
        );
        assert!(shows_with_unknown_seasons(&recent, &releases).is_empty());
    }

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
            split_season_suffix("Tensei Shitara Ken Deshita S2"),
            Some(("Tensei Shitara Ken Deshita", 2))
        );
        assert_eq!(split_season_suffix("Sample S12"), Some(("Sample", 12)));
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
            assert_eq!(split_season_suffix(title), None, "{title:?}");
        }
    }

    #[test]
    fn no_animation_result_means_no_match() {
        let results = results(r#"[{ "id": 1, "name": "Drama", "genre_ids": [18] }]"#);

        assert!(select_match(&results).is_none());
        assert!(select_match(&[]).is_none());
    }
}
