SELECT MAX(updated_at) AS updated_at
FROM (SELECT MAX(updated_at) AS updated_at
      FROM batch_download
      UNION
      SELECT MAX(updated_at) AS updated_at
      FROM episode_download
      UNION
      SELECT MAX(updated_at) AS updated_at
      FROM movie_download) AS variants