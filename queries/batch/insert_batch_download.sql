INSERT INTO download (variant, provider, title, start_index, end_index, created_at, updated_at)
VALUES ('batch', $1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING
RETURNING id