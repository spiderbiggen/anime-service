SELECT d.id, d.updated_at, ARRAY_AGG(dr.resolution) AS resolutions
FROM download d
         JOIN download_resolution dr ON d.id = dr.download_id
WHERE d.variant = 'movie'
  AND d.provider = $1
  AND d.title = $2
GROUP BY d.id