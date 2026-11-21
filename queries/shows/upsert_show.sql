INSERT INTO show (tmdb_id, data, fetched_at)
VALUES ($1, $2, now())
ON CONFLICT (tmdb_id) DO UPDATE
    SET data       = EXCLUDED.data,
        fetched_at = now()
