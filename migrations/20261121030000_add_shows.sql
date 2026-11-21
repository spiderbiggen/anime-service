-- TMDB show metadata, cached as the summary that is attached to download groups.
CREATE TABLE show
(
    tmdb_id    INTEGER PRIMARY KEY,
    data       JSONB       NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Maps a release title to a TMDB show. `tmdb_id` is NULL when no match was found. Rows with
-- source 'manual' are never touched by the resolver, so they can be used to fix a wrong match:
--   INSERT INTO title_match (title, tmdb_id, source) VALUES ('Some Title', 12345, 'manual')
--   ON CONFLICT (title) DO UPDATE SET tmdb_id = EXCLUDED.tmdb_id, source = 'manual';
-- There is deliberately no foreign key to `show`: the resolver fetches missing shows itself.
CREATE TABLE title_match
(
    title      VARCHAR     PRIMARY KEY,
    tmdb_id    INTEGER,
    source     TEXT        NOT NULL CHECK (source IN ('auto', 'manual')),
    checked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX title_match_tmdb_id_idx ON title_match (tmdb_id);
