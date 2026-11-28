//! A fake anime-service for testing a consumer of the SSE stream, with no Nyaa, TMDB or database.
//!
//! ```sh
//! cargo run --bin mock-stream -- [port] [seconds between events] [seconds between keep-alives]
//! ```
//!
//! It serves `GET /v1/downloads/updates` (and `GET /health`) on the loopback interface. An event
//! is sent right after connecting and then every few seconds, cycling through every kind of
//! release. The events are built from the real [`DownloadGroup`] model, so they have exactly the
//! shape the real service sends. Every third event is written in two chunks with a pause in
//! between, so a client's SSE parser has to cope with an event that is split across reads.

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::Stream;
use jiff::Timestamp;
use tokio::net::TcpListener;
use tokio::time::{Instant, interval, interval_at, sleep};
use tracing::info;
use tracing_subscriber::prelude::*;

use anime_service::models::{
    Download, DownloadGroup, DownloadVariant, Episode, EpisodeRange, SeasonRef, ShowSummary,
    TmdbEpisodes, TmdbMatch,
};

const DEFAULT_PORT: u16 = 8000;
const DEFAULT_EVENT_EVERY: Duration = Duration::from_secs(10);
const DEFAULT_KEEPALIVE_EVERY: Duration = Duration::from_secs(15);
/// How long the first half of a split event is held back.
const SPLIT_PAUSE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy)]
struct Config {
    event_every: Duration,
    keepalive_every: Duration,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut args = std::env::args().skip(1);
    let port = parse_arg(args.next(), DEFAULT_PORT, "port")?;
    let event_every = parse_seconds(args.next(), DEFAULT_EVENT_EVERY, "seconds between events")?;
    let keepalive_every = parse_seconds(
        args.next(),
        DEFAULT_KEEPALIVE_EVERY,
        "seconds between keep-alives",
    )?;

    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    info!(
        "mock stream on http://{}/v1/downloads/updates",
        listener.local_addr()?
    );
    axum::serve(
        listener,
        router(Config {
            event_every,
            keepalive_every,
        }),
    )
    .await?;
    Ok(())
}

fn parse_arg<T: std::str::FromStr>(value: Option<String>, default: T, name: &str) -> Result<T> {
    value.map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid {name}: {value}"))
    })
}

fn parse_seconds(value: Option<String>, default: Duration, name: &str) -> Result<Duration> {
    let seconds: f64 = parse_arg(value, default.as_secs_f64(), name)?;
    Duration::try_from_secs_f64(seconds)
        .ok()
        .filter(|duration| !duration.is_zero())
        .with_context(|| format!("{name} must be greater than zero"))
}

fn router(config: Config) -> Router {
    Router::new()
        .route("/health", get(|| async { StatusCode::NO_CONTENT }))
        .route("/v1/downloads/updates", get(updates))
        .with_state(config)
}

async fn updates(State(config): State<Config>) -> impl IntoResponse {
    (
        [
            (CONTENT_TYPE, "text/event-stream"),
            (CACHE_CONTROL, "no-cache"),
        ],
        Body::from_stream(events(config)),
    )
}

/// Logs when a client goes away, which is when the response stream is dropped.
struct Connection;

impl Connection {
    fn open() -> Self {
        info!("client connected");
        Self
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        info!("client disconnected");
    }
}

fn events(config: Config) -> impl Stream<Item = Result<Bytes, Infallible>> {
    async_stream::stream! {
        let _connection = Connection::open();
        // The first tick of an interval completes immediately, so an event goes out on connect.
        let mut event_tick = interval(config.event_every);
        let mut keepalive_tick =
            interval_at(Instant::now() + config.keepalive_every, config.keepalive_every);
        let mut sent = 0_usize;
        loop {
            tokio::select! {
                _ = event_tick.tick() => {
                    let cases = cases(Timestamp::now());
                    let case = sent % cases.len();
                    let payload = frame(&cases[case]);
                    let split = sent % 3 == 2;
                    if split {
                        let (first, second) = payload.split_at(payload.len() / 2);
                        yield Ok(Bytes::copy_from_slice(first));
                        sleep(SPLIT_PAUSE).await;
                        yield Ok(Bytes::copy_from_slice(second));
                    } else {
                        yield Ok(Bytes::from(payload));
                    }
                    info!(case, split, "sent an event");
                    sent += 1;
                }
                _ = keepalive_tick.tick() => {
                    yield Ok(Bytes::from_static(b": keep-alive\n\n"));
                }
            }
        }
    }
}

/// Formats a release as an SSE frame, exactly like the real endpoint.
fn frame(group: &DownloadGroup) -> Vec<u8> {
    let json = serde_json::to_string(group).unwrap_or_else(|error| {
        // Only reachable if the model stops being serializable, which the tests would catch.
        format!(r#"{{"error":"{error}"}}"#)
    });
    format!("event: download\ndata: {json}\n\n").into_bytes()
}

fn show() -> ShowSummary {
    ShowSummary {
        tmdb_id: 1001,
        tvdb_id: Some(424_242),
        name: "Sample Show".into(),
        original_name: Some("サンプルショー".into()),
        romaji_name: Some("Sanpuru Shou".into()),
        overview: "A made-up synopsis, long enough to see how an embed description looks.".into(),
        poster_url: Some("https://image.tmdb.org/t/p/w500/sample-poster.jpg".into()),
        backdrop_url: Some("https://image.tmdb.org/t/p/w780/sample-backdrop.jpg".into()),
    }
}

fn group(
    title: &str,
    variant: DownloadVariant,
    resolutions: &[u16],
    show: Option<ShowSummary>,
    now: Timestamp,
) -> DownloadGroup {
    DownloadGroup {
        title: title.to_owned(),
        variant,
        created_at: now,
        updated_at: now,
        downloads: resolutions
            .iter()
            .map(|resolution| Download {
                comments: format!("https://nyaa.si/view/{resolution}"),
                resolution: *resolution,
                torrent: format!("https://nyaa.si/download/{resolution}.torrent"),
                file_name: format!("[SubsPlease] {title} ({resolution}p).mkv"),
                published_date: now,
            })
            .collect(),
        show,
        tmdb: None,
    }
}

/// Adds TMDB numbering to a release, in season 2 so that it differs from the release's own.
fn numbered(group: DownloadGroup, episodes: TmdbEpisodes) -> DownloadGroup {
    DownloadGroup {
        tmdb: Some(TmdbMatch {
            season: SeasonRef { number: 2 },
            episodes,
        }),
        ..group
    }
}

fn episode(episode: u32) -> DownloadVariant {
    DownloadVariant::Episode(Episode {
        episode,
        decimal: None,
        version: None,
        extra: None,
    })
}

/// One release per interesting case, starting with one that has everything. Consumers that skip
/// releases without a 1080p download should skip the third one.
fn cases(now: Timestamp) -> Vec<DownloadGroup> {
    vec![
        group(
            "Sample Show",
            DownloadVariant::Episode(Episode {
                episode: 10,
                decimal: Some(5),
                version: Some(2),
                extra: Some("END".into()),
            }),
            &[480, 720, 1080],
            Some(show()),
            now,
        ),
        numbered(
            group(
                "Sample Show",
                episode(7),
                &[480, 720, 1080],
                Some(show()),
                now,
            ),
            TmdbEpisodes::Episode { number: 19 },
        ),
        group("Sample Show", episode(8), &[480, 720], None, now),
        group(
            "Sample Show",
            DownloadVariant::Episode(Episode {
                episode: 9,
                decimal: Some(5),
                version: Some(2),
                extra: Some("END".into()),
            }),
            &[1080],
            None,
            now,
        ),
        numbered(
            group(
                "Sample Show",
                DownloadVariant::Batch(1..=12),
                &[1080],
                Some(show()),
                now,
            ),
            TmdbEpisodes::Episodes(EpisodeRange { start: 13, end: 24 }),
        ),
        group("Sample Movie", DownloadVariant::Movie, &[1080], None, now),
        group("Unmatched Show", episode(1), &[1080], None, now),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn json(group: &DownloadGroup) -> Value {
        serde_json::to_value(group).expect("serializes")
    }

    #[test]
    fn every_kind_of_release_is_covered() {
        let cases: Vec<_> = cases(Timestamp::now()).iter().map(json).collect();

        let variants: Vec<_> = cases.iter().map(|case| case["variant"].as_str()).collect();
        for variant in ["episode", "batch", "movie"] {
            assert!(variants.contains(&Some(variant)), "missing {variant}");
        }
        assert!(cases.iter().any(|case| case.get("show").is_some()));
        assert!(cases.iter().any(|case| case.get("show").is_none()));
        assert!(
            cases
                .iter()
                .any(|case| case["tmdb"].get("episode").is_some())
        );
        assert!(
            cases
                .iter()
                .any(|case| case["tmdb"].get("episodes").is_some())
        );
        assert!(cases.iter().any(|case| case.get("tmdb").is_none()));
        assert!(
            cases.iter().any(|case| {
                let resolutions: Vec<_> = case["downloads"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|download| download["resolution"].as_u64())
                    .collect();
                resolutions == [Some(480), Some(720), Some(1080)]
                    && case["decimal"] == 5
                    && case["version"] == 2
                    && case["extra"] == "END"
                    && case.get("show").is_some()
            }),
            "missing an episode with every field, a show and all three resolutions"
        );
        assert!(
            cases
                .iter()
                .any(|case| case["episode"] == 9 && case["decimal"] == 5 && case["extra"] == "END")
        );
        assert!(
            cases
                .iter()
                .any(|case| case["start"] == 1 && case["end"] == 12)
        );
    }

    #[test]
    fn a_frame_is_a_named_event_with_the_json_as_data() {
        let frame = String::from_utf8(frame(&cases(Timestamp::now())[0])).expect("utf-8");

        let (head, data) = frame.split_once("\ndata: ").expect("event and data lines");
        assert_eq!(head, "event: download");
        let json: Value = serde_json::from_str(data.trim_end()).expect("json data");
        assert_eq!(json["title"], "Sample Show");
        assert_eq!(json["downloads"][2]["resolution"], 1080);
        assert!(frame.ends_with("\n\n"));
    }

    #[test]
    fn arguments_are_validated() {
        assert_eq!(parse_arg(None, 8000_u16, "port").expect("default"), 8000);
        assert!(parse_arg(Some("nope".into()), 8000_u16, "port").is_err());
        assert!(parse_seconds(Some("0".into()), Duration::from_secs(1), "x").is_err());
        assert!(parse_seconds(Some("-3".into()), Duration::from_secs(1), "x").is_err());
        assert_eq!(
            parse_seconds(Some("0.5".into()), Duration::from_secs(1), "x").expect("valid"),
            Duration::from_millis(500)
        );
    }

    /// Serves the mock on an ephemeral port and reads it like a real client would.
    #[tokio::test]
    async fn streams_events_including_split_ones_and_keep_alives() {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("bind");
        let address = listener.local_addr().expect("address");
        let config = Config {
            event_every: Duration::from_millis(40),
            keepalive_every: Duration::from_millis(25),
        };
        tokio::spawn(async move { axum::serve(listener, router(config)).await });

        let mut response = reqwest::get(format!("http://{address}/v1/downloads/updates"))
            .await
            .expect("connects");
        assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");

        let mut body = String::new();
        let deadline = Instant::now() + Duration::from_secs(4);
        // Four events: the third is split across two chunks.
        while body.matches("event: download").count() < 4 && Instant::now() < deadline {
            if let Ok(Some(chunk)) = tokio::time::timeout(Duration::from_secs(2), response.chunk())
                .await
                .expect("data in time")
            {
                body.push_str(std::str::from_utf8(&chunk).expect("utf-8"));
            }
        }

        let frames: Vec<_> = body
            .split("\n\n")
            .filter(|frame| !frame.is_empty())
            .collect();
        let events: Vec<_> = frames
            .iter()
            .filter(|frame| frame.starts_with("event: download"))
            .collect();
        assert!(events.len() >= 4, "got {} events", events.len());
        for event in events {
            let data = event.split_once("\ndata: ").expect("data line").1;
            serde_json::from_str::<Value>(data).expect("every event is valid json");
        }
        assert!(
            frames.contains(&": keep-alive"),
            "no keep-alive in {frames:?}"
        );
    }
}
