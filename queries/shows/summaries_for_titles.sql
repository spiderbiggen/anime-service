SELECT tm.title AS "title!",
       s.data   AS "data!: sqlx::types::Json<crate::models::ShowSummary>"
FROM title_match tm
         JOIN show s ON s.tmdb_id = tm.tmdb_id
WHERE tm.title = ANY ($1)
