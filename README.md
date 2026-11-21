# anime-service

Polls [Nyaa](https://nyaa.si) for new anime releases, stores them in PostgreSQL, and serves them over REST, with Server-Sent Events for live updates, on port `8000`. Show metadata comes from [TMDB](https://www.themoviedb.org).

## API

REST endpoints (everything except the health check is under `/v1`):

| Endpoint                                              | Description                                   |
|-------------------------------------------------------|-----------------------------------------------|
| `GET /health`                                         | Health check (204)                            |
| `GET /v1/shows?query=`                                | Search TV shows (also `page`, `language`)     |
| `GET /v1/shows/{id}`                                  | Show details by TMDB id, with `tvdb_id`       |
| `GET /v1/downloads`                                   | Latest downloads (25), all variants           |
| `GET /v1/downloads/{batches,episodes,movies}`         | Same, for one variant                         |
| `GET /v1/downloads[/{batches,episodes,movies}]/updates` | SSE stream of new downloads                 |

The downloads list endpoints (not `/updates`) accept an optional case-insensitive `?title=` prefix filter.

Live updates are Server-Sent Events: each new release arrives as an event named `download` whose data is the same JSON as the list endpoints.

## Show matching

When a new release arrives, the service looks its title up on TMDB (the top result tagged Animation) and attaches the show to the download as a `show` object with `tmdb_id`, `tvdb_id`, `name`, `overview`, `poster_url` and `backdrop_url`. It appears in the downloads endpoints and in the SSE events. Matching is best-effort and takes at most about 5 seconds, so a release is never held back: if TMDB is slow or has no match, the download is sent without a `show`. Titles are only looked up when a new release for them arrives, so older downloads get a `show` later.

Matches live in the `title_match` table and show data in the `show` table. A title with no match is retried after a week, and show data older than 30 days is refreshed the next time a release for it arrives. To fix a wrong match, add a manual row, which the resolver never overwrites:

```sql
INSERT INTO title_match (title, tmdb_id, source) VALUES ('Some Title', 12345, 'manual')
ON CONFLICT (title) DO UPDATE SET tmdb_id = EXCLUDED.tmdb_id, source = 'manual';
```

## Running

Set `PG_HOST`, `PG_PORT`, `PG_USER`, `PG_PASS` and `PG_DATABASE`, plus `TMDB_ACCESS_TOKEN` (a TMDB API read access token; the service refuses to start without it). A `.env` file works locally. Migrations run on startup. Use `RUST_LOG` to control logging.

```sh
cargo run --bin anime-service     # poller + REST/SSE
```

Needs Postgres. sqlx checks queries at compile time: set `DATABASE_URL`, or rely on the offline cache in `.sqlx/` (refresh it with `cargo sqlx prepare --workspace`).

For testing a consumer without waiting for Nyaa to publish something, two optional settings control the poller: `POLL_INTERVAL_SECS` (default 300) is how often Nyaa is polled, aligned to multiples of the interval, and `POLL_LOOKBACK_SECS` makes the first poll replay the releases published within that many seconds instead of continuing after the newest stored one. Replayed releases that are already stored are left alone, and all of them are broadcast over SSE, so this gives a consumer a burst of events within a minute (limited to what Nyaa's feed still lists):

```sh
POLL_INTERVAL_SECS=60 POLL_LOOKBACK_SECS=86400 cargo run --bin anime-service
```

To test a consumer of the SSE stream without Nyaa, TMDB or a database, run the mock server instead of the service:

```sh
cargo run --bin mock-stream -- [port] [seconds between events] [seconds between keep-alives]
```

It listens on `127.0.0.1:8000` by default and serves `/v1/downloads/updates` (and `/health`). It sends an event right away and then every 10 seconds, cycling through an episode with every field (`decimal`, `version`, `extra`, a `show` and 480p, 720p and 1080p downloads), a plain episode with a `show` and all three sizes, an episode without a 1080p download, an episode with `decimal`, `version` and `extra` but only 1080p, a batch, a movie and an episode without a `show`. The events are built from the service's own models, so they have the real shape, and every third event is split across two chunks to exercise the client's SSE parser.

Pushing a `v*.*.*` tag builds musl binaries (amd64, arm64) and publishes a Docker image to GHCR.

## Layout

`nyaa/` and `tmdb/` are the API clients and the root crate is the service itself. See `AGENTS.md` for the file-level details.

## Credits

This product uses the TMDB API but is not endorsed or certified by TMDB.
