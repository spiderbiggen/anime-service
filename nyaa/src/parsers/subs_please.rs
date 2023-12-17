use std::ops::RangeInclusive;
use std::str::FromStr;

use crate::parsers::{ParsedDownload, ParsedDownloadVariant, ParsedEpisode};
use winnow::ascii::{alphanumeric1, digit1, space1};
use winnow::combinator::{delimited, opt, permutation, preceded, rest, separated_pair};
use winnow::error::InputError;
use winnow::token::take_till;
use winnow::{PResult, Parser};

fn parse_digits<'s, N: FromStr>(input: &mut &'s str) -> PResult<N, InputError<&'s str>> {
    digit1.parse_to().parse_next(input)
}

fn resolution<'s>(input: &mut &'s str) -> PResult<u16, InputError<&'s str>> {
    delimited('(', parse_digits, "p)").parse_next(input)
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
    match hash_or_batch {
        "Batch" => {
            let full_title = take_till(0.., |c| c == '(').parse_next(&mut value_ref)?;
            let range = batch_range.parse_next(&mut value_ref)?;
            space1.parse_next(&mut value_ref)?;
            let resolution = resolution.parse_next(&mut value_ref)?;
            let title = full_title.trim();
            Ok(ParsedDownload {
                source,
                title,
                download_type: ParsedDownloadVariant::Batch(range),
                resolution,
            })
        }
        _ => {
            let full_title = take_till(0.., |c| c == '(').parse_next(&mut value_ref)?;
            let resolution = resolution.parse_next(&mut value_ref)?;
            let full_title = full_title.trim();
            match full_title.rfind("- ") {
                None => Ok(ParsedDownload {
                    source,
                    title: full_title,
                    download_type: ParsedDownloadVariant::Movie,
                    resolution,
                }),
                Some(index) => {
                    let mut slice = &full_title[index + 2..];
                    match parse_episode_identifier(&mut slice)? {
                        None => Ok(ParsedDownload {
                            source,
                            title: full_title,
                            download_type: ParsedDownloadVariant::Movie,
                            resolution,
                        }),
                        Some(ep) => Ok(ParsedDownload {
                            source,
                            title: full_title[..index].trim(),
                            download_type: ParsedDownloadVariant::Episode(ep),
                            resolution,
                        }),
                    }
                }
            }
        }
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
