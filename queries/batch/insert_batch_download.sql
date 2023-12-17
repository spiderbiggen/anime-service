INSERT INTO batch_download (provider, title, start_index, end_index, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING
RETURNING id