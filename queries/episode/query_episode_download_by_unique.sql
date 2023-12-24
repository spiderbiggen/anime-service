SELECT d.id, d.updated_at, array_agg(dr.resolution) as resolutions
FROM download d
         inner JOIN download_resolution dr on d.id = dr.download_id
WHERE d.variant = 'episode'
  AND d.provider = $1
  AND d.title = $2
  AND d.episode = $3
  AND COALESCE(d.decimal, -1) = COALESCE($4, -1)
  AND COALESCE(d.version, -1) = COALESCE($5, -1)
  AND COALESCE(d.extra, '') = COALESCE($6, '')
GROUP BY id