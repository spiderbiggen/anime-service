-- TMDB seasons, cached to work out which season and episode a release is.
CREATE TABLE season
(
    tmdb_id       INTEGER     NOT NULL,
    season_number INTEGER     NOT NULL,
    data          JSONB       NOT NULL,
    fetched_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tmdb_id, season_number)
);

-- A show's season numbers and episode counts. They are not part of `data`, which is sent along
-- with downloads. NULL until the show is fetched again.
ALTER TABLE show
    ADD COLUMN seasons JSONB;

-- A manual correction of a title's season numbering: a release's episode N is episode
-- N + `episode_offset` of TMDB season `season_number`. The resolver never changes these.
--   UPDATE title_match SET season_number = 1, episode_offset = 28
--   WHERE title = 'Sousou no Frieren S2';
ALTER TABLE title_match
    ADD COLUMN season_number  INTEGER,
    ADD COLUMN episode_offset INTEGER,
    ADD CONSTRAINT complete_season_override
        CHECK ((season_number IS NULL) = (episode_offset IS NULL));

-- Mark cached shows stale so their seasons are filled in the next time a release for them arrives
-- (see `show_ids_to_refresh`).
UPDATE show
SET fetched_at = now() - INTERVAL '31 days';
