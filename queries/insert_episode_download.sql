INSERT INTO episode_download (title, episode, decimal, version, extra, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT DO NOTHING
RETURNING id