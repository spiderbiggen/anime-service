SELECT t.title AS "title!"
FROM UNNEST($1::text[]) AS t(title)
         LEFT JOIN title_match tm ON tm.title = t.title
WHERE tm.title IS NULL
   OR (tm.tmdb_id IS NULL AND tm.source = 'auto' AND tm.checked_at < now() - INTERVAL '7 days')
