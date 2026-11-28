INSERT INTO show (tmdb_id, data, seasons, fetched_at)
VALUES ($1, $2, $3, now())
ON CONFLICT (tmdb_id) DO UPDATE
    SET data       = EXCLUDED.data,
        seasons    = EXCLUDED.seasons,
        fetched_at = now()
