INSERT INTO download_resolution (download_id, resolution, torrent, file_name, comments, magnet, created_at)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT DO NOTHING