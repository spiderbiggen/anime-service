# AGENTS.md

Guidance for AI coding agents working in this repo. See `README.md` for the overview and API.

## Commands

```sh
cargo fmt --all
cargo test --workspace
cargo clippy --all-targets --workspace -- -D warnings   # CI fails on any warning
cargo build --locked
```

CI (`.github/workflows/ci.yml`) runs fmt check, tests, and clippy. Run all three before finishing.

## Layout

- Cargo workspace: root crate `anime-service` plus the API clients `nyaa` and `tmdb`.
- `src/controllers/rest.rs` handles transport (REST and SSE); `src/datasource/repository/` holds DB access; `src/jobs/poller.rs` is the Nyaa poller and `src/jobs/resolver.rs` matches release titles to TMDB shows and fetches their seasons; `src/seasons.rs` works out a release's TMDB season and episode numbers (no I/O); `src/state.rs` holds `AppState`.
- `src/bin/`: `anime-service` is the service; `mock-stream` is a fake SSE stream for testing consumers (it builds its events from the real models, so keep it compiling when the models change).
- Router wiring lives in `src/lib.rs`.

## Conventions

- Avoid `unwrap` and `expect` outside `main`. Propagate errors instead.
- Prefer iterators over manual loops, unless the iterator chain becomes hard to read.
- Write good doc comments (`///`) on public items. Keep inline code comments minimal and only for non-obvious "why".
- Use `tracing` (`#[instrument]`, `info!`, etc.), not `println!`.
- The Rust toolchain is pinned in `rust-toolchain.toml` and CI installs exactly that version. Renovate bumps it in its own PR, which is where new clippy lints should be fixed. Don't switch CI back to floating `stable`.
- `CLAUDE.md` only contains `@AGENTS.md` so Claude Code loads this file. Keep the guidance here, not there.

## Database

- SQL lives in `queries/*.sql` and is checked at compile time by sqlx. Migrations are in `migrations/` and run on startup.
- Never edit an existing migration; add a new one.
- After changing a query or migration, refresh the offline cache (`cargo sqlx prepare --workspace`) and commit the `.sqlx/` changes. Otherwise CI, which has no database, will fail to build.
- Dates and times use `jiff` (`jiff::Timestamp`), not `chrono`. sqlx 0.9 has no `jiff` support, so at the SQL boundary use the `jiff_sqlx` wrappers: alias result columns as `AS "col: jiff_sqlx::Timestamp"` in the `.sql` file and convert with `.to_jiff()`, and bind parameters with `.to_sqlx() as _`.
- Regenerating the cache needs a running Postgres with the migrations applied: set `DATABASE_URL` and run `cargo sqlx prepare --workspace -- --all-targets`.
- Config comes from `PG_*` env vars. `.env` is for local use. Do not print or commit its values.

## Git

- Commit messages follow Conventional Commits (`feat:`, `fix:`, `chore(deps):`).
- Dependency bumps come from Renovate. Don't hand-edit `Cargo.lock` beyond what cargo produces.
- Releases are cut by pushing a `vX.Y.Z` tag, which triggers the Docker image build. Don't tag unless asked.
