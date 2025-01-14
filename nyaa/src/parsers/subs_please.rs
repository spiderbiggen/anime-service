use std::ops::RangeInclusive;
use std::str::FromStr;

use winnow::ascii::{alphanumeric1, digit1};
use winnow::combinator::{alt, delimited, opt, permutation, preceded, separated_pair};
use winnow::error::{ErrMode, ErrorKind, InputError, ParserError};
use winnow::token::{rest, take_till, take_until};
use winnow::{PResult, Parser};

use crate::parsers::{ParsedDownload, ParsedDownloadVariant, ParsedEpisode};

fn parse_digits<'s, N: FromStr>(input: &mut &'s str) -> PResult<N, InputError<&'s str>> {
    digit1.parse_to().parse_next(input)
}

fn resolution<'s>(input: &mut &'s str) -> PResult<u16, InputError<&'s str>> {
    delimited('(', parse_digits, "p)").parse_next(input)
}

fn parse_resolution<'s>(input: &mut &'s str) -> PResult<u16, InputError<&'s str>> {
    let full_title = alt((
        take_until(0.., "(1080p)"),
        take_until(0.., "(720p)"),
        take_until(0.., "(480p)"),
    ))
    .parse_next(input)?;
    let resolution = resolution.parse_next(input)?;
    *input = full_title.trim();
    Ok(resolution)
}

fn parse_episode_identifier<'s>(
    input: &mut &'s str,
) -> PResult<Option<ParsedEpisode<'s>>, InputError<&'s str>> {
    let Some(number) = opt(parse_digits).parse_next(input)? else {
        return Ok(None);
    };
    let (decimal, version) = permutation((
        opt(preceded('.', parse_digits)),
        opt(preceded('v', parse_digits)),
    ))
    .parse_next(input)?;
    let extra = opt(alphanumeric1).parse_next(input)?;
    Ok(Some(ParsedEpisode {
        number,
        decimal,
        version,
        extra,
    }))
}

fn batch_range<'s>(input: &mut &'s str) -> PResult<RangeInclusive<u32>, InputError<&'s str>> {
    delimited('(', separated_pair(parse_digits, '-', parse_digits), ")")
        .map(|(left, right)| left..=right)
        .parse_next(input)
}

fn parse_batch_range<'s>(input: &mut &'s str) -> PResult<RangeInclusive<u32>, InputError<&'s str>> {
    let Some(index) = input.rfind('(') else {
        return Err(ErrMode::Backtrack(InputError::from_error_kind(
            input,
            ErrorKind::Assert,
        )));
    };
    let mut range = &input[index..];
    let batch_range = batch_range.parse_next(&mut range)?;
    *input = &input[..index];
    Ok(batch_range)
}

fn square_brackets<'s>(input: &mut &'s str) -> PResult<&'s str, InputError<&'s str>> {
    delimited('[', take_till(0.., |c| c == ']'), ']').parse_next(input)
}

fn parse_file_end<'s>(input: &mut &'s str) -> PResult<&'s str, InputError<&'s str>> {
    let tag = square_brackets.parse_next(input)?;
    rest.verify(|rest: &str| rest.is_empty() || rest == ".mkv")
        .parse_next(input)?;
    Ok(tag)
}

// TODO cleanup
pub(crate) fn parse_filename(value: &str) -> PResult<ParsedDownload, InputError<&str>> {
    let mut value_ref = value;
    let source = square_brackets.parse_next(&mut value_ref)?;
    let full_title = take_till(0.., |c| c == '[').parse_next(&mut value_ref)?;
    let hash_or_batch = parse_file_end.parse_next(&mut value_ref)?;
    value_ref = full_title.trim();
    let resolution = parse_resolution(&mut value_ref)?;
    match hash_or_batch {
        "Batch" => {
            let range = parse_batch_range.parse_next(&mut value_ref)?;
            let title = value_ref.trim();
            Ok(ParsedDownload {
                source,
                title,
                download_type: ParsedDownloadVariant::Batch(range),
                resolution,
            })
        }
        _ => match value_ref.rfind("- ") {
            None => Ok(ParsedDownload {
                source,
                title: value_ref,
                download_type: ParsedDownloadVariant::Movie,
                resolution,
            }),
            Some(index) => {
                let mut slice = value_ref[index..].trim_start_matches(['-', ' ', '#']);
                match parse_episode_identifier(&mut slice)? {
                    None => Ok(ParsedDownload {
                        source,
                        title: value_ref,
                        download_type: ParsedDownloadVariant::Movie,
                        resolution,
                    }),
                    Some(ep) => Ok(ParsedDownload {
                        source,
                        title: value_ref[..index].trim(),
                        download_type: ParsedDownloadVariant::Episode(ep),
                        resolution,
                    }),
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filename_batch() {
        let input = "[SubsPlease] Arknights - Reimei Zensou (01-08) (1080p) [Batch]";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "Arknights - Reimei Zensou",
            download_type: ParsedDownloadVariant::Batch(1..=8),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_batch_with_extra_brackets() {
        let input = "[SubsPlease] Urusei Yatsura (2022) (01-08) (1080p) [Batch]";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "Urusei Yatsura (2022)",
            download_type: ParsedDownloadVariant::Batch(1..=8),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_movie() {
        let input = "[SubsPlease] Boku no Hero Academia - UA Heroes Battle (720p) [F3A40F62].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "Boku no Hero Academia - UA Heroes Battle",
            download_type: ParsedDownloadVariant::Movie,
            resolution: 720,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_movie_with_extra_brackets() {
        let input = "[SubsPlease] Urusei Yatsura (2022) (1080p) [F3A40F62].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "Urusei Yatsura (2022)",
            download_type: ParsedDownloadVariant::Movie,
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_episode() {
        let input = "[SubsPlease] 16bit Sensation - Another Layer - 10 (1080p) [2A96C634].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "16bit Sensation - Another Layer",
            download_type: ParsedDownloadVariant::Episode(ParsedEpisode {
                number: 10,
                decimal: None,
                version: None,
                extra: None,
            }),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_episode_with_extra_brackets() {
        let input = "[SubsPlease] Urusei Yatsura (2022) - 25 (1080p) [C0AF019E].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "Urusei Yatsura (2022)",
            download_type: ParsedDownloadVariant::Episode(ParsedEpisode {
                number: 25,
                decimal: None,
                version: None,
                extra: None,
            }),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_episode_with_decimal() {
        let input = "[SubsPlease] 16bit Sensation - Another Layer - 10.5 (1080p) [2A96C634].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "16bit Sensation - Another Layer",
            download_type: ParsedDownloadVariant::Episode(ParsedEpisode {
                number: 10,
                decimal: Some(5),
                version: None,
                extra: None,
            }),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_episode_with_version() {
        let input = "[SubsPlease] 16bit Sensation - Another Layer - 10v2 (1080p) [2A96C634].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "16bit Sensation - Another Layer",
            download_type: ParsedDownloadVariant::Episode(ParsedEpisode {
                number: 10,
                decimal: None,
                version: Some(2),
                extra: None,
            }),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_episode_with_decimal_and_version() {
        let input = "[SubsPlease] 16bit Sensation - Another Layer - 10.5v2 (1080p) [2A96C634].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "16bit Sensation - Another Layer",
            download_type: ParsedDownloadVariant::Episode(ParsedEpisode {
                number: 10,
                decimal: Some(5),
                version: Some(2),
                extra: None,
            }),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_episode_with_extra() {
        let input = "[SubsPlease] 16bit Sensation - Another Layer - 10Extra (1080p) [2A96C634].mkv";
        let expected = ParsedDownload {
            source: "SubsPlease",
            title: "16bit Sensation - Another Layer",
            download_type: ParsedDownloadVariant::Episode(ParsedEpisode {
                number: 10,
                decimal: None,
                version: None,
                extra: Some("Extra"),
            }),
            resolution: 1080,
        };
        let result = parse_filename(input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_parse_filename_invalid_batch() {
        let input = "[SubsPlease] Arknights - Reimei Zensou (0108) (1080p) [Batch]";
        let result = parse_filename(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_filename_invalid_resolution() {
        let input = "[SubsPlease] 16bit Sensation - Another Layer - 10 (Invalid) [2A96C634].mkv";
        let result = parse_filename(input);
        assert!(result.is_err());
    }
}
