SELECT tm.title                                  AS "title!",
       s.tmdb_id                                 AS "tmdb_id!",
       tm.season_number,
       tm.episode_offset,
       s.seasons                                 AS "seasons: sqlx::types::Json<Vec<crate::seasons::SeasonCount>>",
       (s.fetched_at < now() - INTERVAL '1 day') AS "show_is_old!"
FROM title_match tm
         JOIN show s ON s.tmdb_id = tm.tmdb_id
WHERE tm.title = ANY ($1)
