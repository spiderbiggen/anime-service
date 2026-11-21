# anime-service

Polls [Nyaa](https://nyaa.si) for new anime releases, stores them in PostgreSQL, and serves them over REST (with SSE) and gRPC streaming on port `8000`. Show metadata comes from [Kitsu](https://kitsu.app).

## API

REST endpoints (everything except the health check is under `/v1`):

| Endpoint                                              | Description                                   |
|-------------------------------------------------------|-----------------------------------------------|
| `GET /health`                                         | Health check (204)                            |
| `GET /v1/shows`, `GET /v1/shows/{id}`                 | Show metadata from Kitsu                      |
| `GET /v1/downloads`                                   | Latest downloads (25), all variants           |
| `GET /v1/downloads/{batches,episodes,movies}`         | Same, for one variant                         |
| `GET /v1/downloads[/{batches,episodes,movies}]/updates` | SSE stream of new downloads                 |

The downloads list endpoints (not `/updates`) accept an optional case-insensitive `?title=` prefix filter.

gRPC: `Downloads.Subscribe` (server-streaming), see `proto/protos/api.v2.proto`.

## Running

Set `PG_HOST`, `PG_PORT`, `PG_USER`, `PG_PASS` and `PG_DATABASE` (a `.env` file works locally). Migrations run on startup. Use `RUST_LOG` to control logging.

```sh
cargo run --bin anime-service     # poller + REST + gRPC
cargo run --bin combined-debug    # polls every minute, looks back one week
cargo run --bin streams           # poller + gRPC only, no database
```

Needs `protoc` and Postgres. sqlx checks queries at compile time: set `DATABASE_URL`, or rely on the offline cache in `.sqlx/` (refresh it with `cargo sqlx prepare --workspace`).

Pushing a `v*.*.*` tag builds musl binaries (amd64, arm64) and publishes a Docker image to GHCR.

## Layout

`nyaa/` and `kitsu/` are the API clients, `proto/` holds the gRPC definitions, and the root crate is the service itself. See `AGENTS.md` for the file-level details.
