INSERT INTO movie_download (provider, title, created_at, updated_at)
VALUES ($1, $2, $3, $4)
ON CONFLICT DO NOTHING
RETURNING id