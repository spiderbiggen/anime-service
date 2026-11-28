SELECT se.tmdb_id,
       se.season_number,
       se.data                                    AS "data!: sqlx::types::Json<crate::seasons::CachedSeason>",
       (se.fetched_at < now() - INTERVAL '1 day') AS "is_old!"
FROM season se
WHERE se.tmdb_id IN (SELECT tm.tmdb_id FROM title_match tm WHERE tm.title = ANY ($1))
