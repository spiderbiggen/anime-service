SELECT ed.id, array_agg(edr.resolution) as resolutions
FROM episode_download ed
         JOIN episode_download_resolution edr on ed.id = edr.episode_download_id
WHERE provider = COALESCE($1, 'SubsPlease')
  AND ed.title = $2
  AND COALESCE(ed.episode, -1) = COALESCE($3, -1)
  AND COALESCE(ed.decimal, -1) = COALESCE($4, -1)
  AND COALESCE(ed.version, -1) = COALESCE($5, -1)
  AND COALESCE(ed.extra, '') = COALESCE($6, '')
GROUP BY id