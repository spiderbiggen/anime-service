SELECT DISTINCT tm.tmdb_id AS "tmdb_id!"
FROM title_match tm
         LEFT JOIN show s ON s.tmdb_id = tm.tmdb_id
WHERE tm.title = ANY ($1)
  AND tm.tmdb_id IS NOT NULL
  AND (s.tmdb_id IS NULL OR s.fetched_at < now() - INTERVAL '30 days')
