SELECT bd.id, bd.updated_at, ARRAY_AGG(bdr.resolution) AS resolutions
FROM batch_download bd
         JOIN batch_download_resolution bdr ON bd.id = bdr.batch_download_id
WHERE provider = $1
  AND bd.title = $2
  AND bd.start_index = $3
  AND bd.end_index = $4
GROUP BY id