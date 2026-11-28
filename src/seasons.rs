//! Works out which TMDB season and episodes a release belongs to.
//!
//! Release numbering can differ from TMDB's, so a match is only reported when it can be checked
//! against the cached season. A missing match is preferred over a wrong one. Nothing here does I/O.

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use tmdb::tv::{SeasonDetails, SeasonSummary};

use crate::jobs::resolver::split_season_suffix;
use crate::models::{DownloadVariant, EpisodeRange, TmdbEpisodes, TmdbMatch, TmdbSeason};

/// How many days before its TMDB air date an episode may be released. Some shows are
/// consistently released more than a week ahead of TMDB's schedule.
const MAX_DAYS_EARLY: i32 = 10;
/// How many days after its TMDB air date a single episode may be released.
const MAX_DAYS_LATE: i32 = 3;

/// How many episodes a season has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SeasonCount {
    pub number: u32,
    pub episode_count: u32,
}

impl From<&SeasonSummary> for SeasonCount {
    fn from(value: &SeasonSummary) -> Self {
        Self {
            number: value.season_number,
            episode_count: value.episode_count,
        }
    }
}

/// The part of a TMDB season that is cached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedSeason {
    pub number: u32,
    pub overview: String,
    pub poster_path: Option<String>,
    pub episodes: Vec<CachedEpisode>,
}

/// An episode of a [`CachedSeason`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedEpisode {
    pub number: u32,
    pub name: String,
    pub air_date: Option<Date>,
}

impl CachedSeason {
    pub(crate) fn episode(&self, number: u32) -> Option<&CachedEpisode> {
        self.episodes
            .iter()
            .find(|episode| episode.number == number)
    }
}

impl CachedEpisode {
    /// Whether TMDB has a title for this episode, rather than nothing or a placeholder.
    pub(crate) fn has_title(&self) -> bool {
        !self.name.is_empty() && self.name != format!("Episode {}", self.number)
    }
}

impl From<SeasonDetails> for CachedSeason {
    fn from(value: SeasonDetails) -> Self {
        Self {
            number: value.season_number,
            overview: value.overview,
            poster_path: value.poster_path,
            episodes: value
                .episodes
                .into_iter()
                .map(|episode| CachedEpisode {
                    number: episode.episode_number,
                    name: episode.name,
                    air_date: episode.air_date,
                })
                .collect(),
        }
    }
}

/// A manual correction for a release title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SeasonOverride {
    /// The TMDB season.
    pub season: u32,
    /// What to add to a release's episode number to get TMDB's. May be negative.
    pub episode_offset: i32,
}

/// Where a release would be on TMDB, before it has been checked against that season.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Target {
    pub season: u32,
    pub first: u32,
    pub last: u32,
}

/// The first and last episode number of a release, if it is of a kind that can be matched:
/// whole-numbered episodes and batches.
pub(crate) fn release_range(variant: &DownloadVariant) -> Option<(u32, u32)> {
    match variant {
        DownloadVariant::Episode(episode) if episode.decimal.is_none() => {
            Some((episode.episode, episode.episode))
        }
        DownloadVariant::Batch(range) if !range.is_empty() => Some((*range.start(), *range.end())),
        _ => None,
    }
}

/// Picks the season and TMDB episode numbers of a release.
///
/// An override wins. Otherwise a title with a suffix such as `S2` means exactly that season. Any
/// other title is numbered from season 1, continuing into later regular seasons.
pub(crate) fn locate(
    title: &str,
    variant: &DownloadVariant,
    season_override: Option<SeasonOverride>,
    counts: &[SeasonCount],
) -> Option<Target> {
    let (first, last) = release_range(variant)?;
    if let Some(SeasonOverride {
        season,
        episode_offset,
    }) = season_override
    {
        return Some(Target {
            season,
            first: first.checked_add_signed(episode_offset)?,
            last: last.checked_add_signed(episode_offset)?,
        });
    }
    match split_season_suffix(title) {
        Some((_, season)) => Some(Target {
            season,
            first,
            last,
        }),
        None => locate_continuous(first, last, counts),
    }
}

/// Converts numbering that runs on across seasons. Specials are not counted, and a range that
/// spans two seasons has no target.
fn locate_continuous(mut first: u32, mut last: u32, counts: &[SeasonCount]) -> Option<Target> {
    for season in 1.. {
        let count = counts.iter().find(|count| count.number == season)?;
        if first <= count.episode_count {
            return (last <= count.episode_count).then_some(Target {
                season,
                first,
                last,
            });
        }
        first -= count.episode_count;
        last = last.checked_sub(count.episode_count)?;
    }
    None
}

/// Checks a target against its season and, if it holds up, returns the match.
///
/// Both ends of the target must exist. Unless the target comes from an override, the release date
/// must also be plausible: a single episode comes out at most [`MAX_DAYS_EARLY`] days before and
/// [`MAX_DAYS_LATE`] days after it airs, and a batch at most [`MAX_DAYS_EARLY`] days before its
/// last episode airs. Dates only ever reject a target, they never pick one.
pub(crate) fn verify(
    target: Target,
    variant: &DownloadVariant,
    released_at: Timestamp,
    overridden: bool,
    season: &CachedSeason,
) -> Option<TmdbMatch> {
    if season.number != target.season {
        return None;
    }
    let first = season.episode(target.first)?;
    let last = season.episode(target.last)?;

    let is_batch = matches!(variant, DownloadVariant::Batch(_));
    if !overridden {
        let released_on = TimeZone::UTC.to_datetime(released_at).date();
        let days_late = last.air_date?.until(released_on).ok()?.get_days();
        if days_late < -MAX_DAYS_EARLY || (!is_batch && days_late > MAX_DAYS_LATE) {
            return None;
        }
    }

    let episodes = if is_batch {
        TmdbEpisodes::Episodes(EpisodeRange {
            start: target.first,
            end: target.last,
        })
    } else {
        TmdbEpisodes::Episode {
            number: first.number,
            name: first.has_title().then(|| first.name.clone()),
        }
    };
    Some(TmdbMatch {
        season: TmdbSeason::new(target.season, season.poster_path.clone(), &season.overview),
        episodes,
    })
}

#[cfg(test)]
mod tests {
    use jiff::ToSpan;
    use jiff::civil::date;

    use super::*;
    use crate::models::Episode;

    fn episode(number: u32) -> DownloadVariant {
        DownloadVariant::Episode(Episode {
            episode: number,
            decimal: None,
            version: None,
            extra: None,
        })
    }

    fn counts(counts: &[(u32, u32)]) -> Vec<SeasonCount> {
        counts
            .iter()
            .map(|&(number, episode_count)| SeasonCount {
                number,
                episode_count,
            })
            .collect()
    }

    /// A season whose episodes `numbers` air weekly, the first of them on `first_air_date`.
    fn weekly(number: u32, numbers: std::ops::RangeInclusive<u32>, first: Date) -> CachedSeason {
        let start = *numbers.start();
        CachedSeason {
            number,
            overview: String::new(),
            poster_path: None,
            episodes: numbers
                .map(|episode| CachedEpisode {
                    number: episode,
                    name: format!("Title of {episode}"),
                    air_date: first.checked_add(i64::from(episode - start).weeks()).ok(),
                })
                .collect(),
        }
    }

    fn noon(date: Date) -> Timestamp {
        TimeZone::UTC
            .to_timestamp(date.at(12, 0, 0, 0))
            .expect("valid time")
    }

    /// Runs both steps, the way callers do.
    fn find(
        title: &str,
        variant: &DownloadVariant,
        released_on: Date,
        season_override: Option<SeasonOverride>,
        counts: &[SeasonCount],
        season: &CachedSeason,
    ) -> Option<TmdbMatch> {
        let target = locate(title, variant, season_override, counts)?;
        verify(
            target,
            variant,
            noon(released_on),
            season_override.is_some(),
            season,
        )
    }

    /// The match expected from a [`weekly`] season.
    fn single(season: u32, number: u32) -> TmdbMatch {
        TmdbMatch {
            season: TmdbSeason::new(season, None, ""),
            episodes: TmdbEpisodes::Episode {
                number,
                name: Some(format!("Title of {number}")),
            },
        }
    }

    #[test]
    fn an_episode_released_eight_days_early_matches() {
        let season = weekly(2, 1..=12, date(2026, 10, 8));

        let found = find(
            "Tensei Shitara Ken Deshita S2",
            &episode(9),
            date(2026, 11, 25),
            None,
            &counts(&[(1, 12), (2, 12)]),
            &season,
        );

        assert_eq!(found, Some(single(2, 9)));
    }

    #[test]
    fn long_running_numbering_stays_in_a_long_first_season() {
        // Episode 57 airs on 2026-11-27.
        let season = weekly(1, 50..=60, date(2026, 10, 9));

        let found = find(
            "Kusuriya no Hitorigoto",
            &episode(57),
            date(2026, 11, 27),
            None,
            &counts(&[(0, 58), (1, 60)]),
            &season,
        );

        assert_eq!(found, Some(single(1, 57)));
    }

    #[test]
    fn a_season_tmdb_merged_needs_an_override() {
        let title = "Sousou no Frieren S2";
        let counts = counts(&[(0, 26), (1, 38)]);
        // Episode 38 airs on 2026-03-27.
        let season = weekly(1, 29..=38, date(2026, 1, 23));
        let released_on = date(2026, 3, 27);

        let without = find(title, &episode(10), released_on, None, &counts, &season);
        let corrected = SeasonOverride {
            season: 1,
            episode_offset: 28,
        };
        let with = find(
            title,
            &episode(10),
            released_on,
            Some(corrected),
            &counts,
            &season,
        );

        assert_eq!(without, None);
        assert_eq!(with, Some(single(1, 38)));
    }

    #[test]
    fn an_override_skips_the_date_check_but_the_episode_must_exist() {
        let season = weekly(2, 1..=12, date(2026, 1, 1));
        let released_on = date(2027, 6, 1);
        let keep = SeasonOverride {
            season: 2,
            episode_offset: 0,
        };

        assert_eq!(
            find("Show", &episode(3), released_on, Some(keep), &[], &season),
            Some(single(2, 3))
        );
        assert_eq!(
            find("Show", &episode(13), released_on, Some(keep), &[], &season),
            None
        );
    }

    #[test]
    fn a_negative_offset_cannot_go_below_zero() {
        let correction = SeasonOverride {
            season: 3,
            episode_offset: -48,
        };

        assert_eq!(
            locate("Show", &episode(49), Some(correction), &[]),
            Some(Target {
                season: 3,
                first: 1,
                last: 1
            })
        );
        assert_eq!(locate("Show", &episode(47), Some(correction), &[]), None);
    }

    #[test]
    fn continuous_batch_numbers_are_converted() {
        let batch = DownloadVariant::Batch(49..=72);
        let season = weekly(3, 1..=24, date(2024, 4, 5));
        let counts = counts(&[(0, 16), (1, 24), (2, 24), (3, 24), (4, 24)]);

        let found = find(
            "Tensei Shitara Slime Datta Ken",
            &batch,
            date(2026, 11, 6),
            None,
            &counts,
            &season,
        );

        assert_eq!(
            found,
            Some(TmdbMatch {
                season: TmdbSeason::new(3, None, ""),
                episodes: TmdbEpisodes::Episodes(EpisodeRange { start: 1, end: 24 }),
            })
        );
    }

    #[test]
    fn a_batch_may_be_partial_but_not_span_two_seasons() {
        let counts = counts(&[(1, 24), (2, 24)]);

        assert_eq!(
            locate("Show", &DownloadVariant::Batch(30..=36), None, &counts),
            Some(Target {
                season: 2,
                first: 6,
                last: 12
            })
        );
        assert_eq!(
            locate("Show", &DownloadVariant::Batch(20..=30), None, &counts),
            None
        );
    }

    #[test]
    fn a_batch_cannot_come_out_long_before_its_last_episode_airs() {
        // The last episode airs on 2026-03-19.
        let season = weekly(1, 1..=12, date(2026, 1, 1));
        let batch = DownloadVariant::Batch(1..=12);
        let counts = counts(&[(1, 12)]);
        let matches =
            |released_on| find("Show", &batch, released_on, None, &counts, &season).is_some();

        assert!(matches(date(2026, 3, 9)));
        assert!(!matches(date(2026, 3, 8)));
        assert!(matches(date(2028, 1, 1)));
    }

    #[test]
    fn a_single_episode_must_be_released_close_to_its_air_date() {
        let air_date = date(2026, 6, 15);
        let season = weekly(1, 1..=1, air_date);
        let counts = counts(&[(1, 1)]);
        let matches =
            |released_on| find("Show", &episode(1), released_on, None, &counts, &season).is_some();

        assert!(matches(date(2026, 6, 5)), "10 days early");
        assert!(!matches(date(2026, 6, 4)), "11 days early");
        assert!(matches(date(2026, 6, 18)), "3 days late");
        assert!(!matches(date(2026, 6, 19)), "4 days late");
    }

    #[test]
    fn an_episode_without_an_air_date_does_not_match() {
        let mut season = weekly(1, 1..=1, date(2026, 6, 15));
        season.episodes[0].air_date = None;

        let found = find(
            "Show",
            &episode(1),
            date(2026, 6, 15),
            None,
            &counts(&[(1, 1)]),
            &season,
        );

        assert_eq!(found, None);
    }

    #[test]
    fn a_season_suffix_never_rolls_into_the_next_season() {
        let target = locate("Show S2", &episode(13), None, &counts(&[(2, 12), (3, 12)]));

        assert_eq!(
            target,
            Some(Target {
                season: 2,
                first: 13,
                last: 13
            })
        );
    }

    #[test]
    fn numbers_beyond_the_known_seasons_have_no_target() {
        assert_eq!(
            locate("Show", &episode(13), None, &counts(&[(1, 12)])),
            None
        );
        assert_eq!(locate("Show", &episode(1), None, &[]), None);
    }

    #[test]
    fn specials_are_not_counted() {
        let target = locate(
            "Show",
            &episode(13),
            None,
            &counts(&[(0, 5), (1, 12), (2, 12)]),
        );

        assert_eq!(
            target,
            Some(Target {
                season: 2,
                first: 1,
                last: 1
            })
        );
    }

    #[test]
    fn decimal_episodes_and_movies_are_not_matched() {
        let decimal = DownloadVariant::Episode(Episode {
            episode: 10,
            decimal: Some(5),
            version: None,
            extra: None,
        });
        let correction = SeasonOverride {
            season: 1,
            episode_offset: 0,
        };

        for variant in [decimal, DownloadVariant::Movie] {
            assert_eq!(locate("Show S1", &variant, None, &[]), None);
            assert_eq!(locate("Show", &variant, Some(correction), &[]), None);
        }
    }

    #[test]
    fn a_match_carries_the_season_details_but_not_a_placeholder_title() {
        let mut season = weekly(2, 11..=11, date(2026, 12, 17));
        season.overview = "About the season".to_owned();
        season.poster_path = Some("/season.jpg".to_owned());
        season.episodes[0].name = "Episode 11".to_owned();

        let found = find(
            "Show S2",
            &episode(11),
            date(2026, 12, 17),
            None,
            &[],
            &season,
        );

        assert_eq!(
            found,
            Some(TmdbMatch {
                season: TmdbSeason::new(2, Some("/season.jpg".to_owned()), "About the season"),
                episodes: TmdbEpisodes::Episode {
                    number: 11,
                    name: None
                },
            })
        );
    }

    #[test]
    fn placeholder_titles_do_not_count_as_titles() {
        let titled = |name: &str| {
            CachedEpisode {
                number: 11,
                name: name.to_owned(),
                air_date: None,
            }
            .has_title()
        };

        assert!(titled("The Floating Island"));
        assert!(titled("Episode 12"));
        assert!(!titled("Episode 11"));
        assert!(!titled(""));
    }
}
