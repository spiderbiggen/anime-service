SELECT ed.id, ed.updated_at, ARRAY_AGG(edr.resolution) AS resolutions
FROM movie_download ed
         JOIN movie_download_resolution edr ON ed.id = edr.movie_download_id
WHERE provider = $1
  AND ed.title = $2
GROUP BY id