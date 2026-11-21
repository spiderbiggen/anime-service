INSERT INTO title_match (title, tmdb_id, source, checked_at)
VALUES ($1, $2, 'auto', now())
ON CONFLICT (title) DO UPDATE
    SET tmdb_id    = EXCLUDED.tmdb_id,
        checked_at = now()
WHERE title_match.source = 'auto'
