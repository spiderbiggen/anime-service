INSERT INTO season (tmdb_id, season_number, data, fetched_at)
VALUES ($1, $2, $3, now())
ON CONFLICT (tmdb_id, season_number) DO UPDATE
    SET data       = EXCLUDED.data,
        fetched_at = now()
