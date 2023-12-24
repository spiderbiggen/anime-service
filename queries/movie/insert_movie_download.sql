INSERT INTO download (variant, provider, title, created_at, updated_at)
VALUES ('movie', $1, $2, $3, $4)
ON CONFLICT DO NOTHING
RETURNING id