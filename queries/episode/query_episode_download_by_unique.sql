SELECT ed.id, ed.updated_at, array_agg(edr.resolution) as resolutions
FROM episode_download ed
         inner JOIN episode_download_resolution edr on ed.id = edr.episode_download_id
WHERE ed.provider = $1
  AND ed.title = $2
  AND ed.episode = $3
  AND COALESCE(ed.decimal, -1) = COALESCE($4, -1)
  AND COALESCE(ed.version, -1) = COALESCE($5, -1)
  AND COALESCE(ed.extra, '') = COALESCE($6, '')
GROUP BY id