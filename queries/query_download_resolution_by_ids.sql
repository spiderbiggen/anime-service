SELECT download_id,
       resolution,
       torrent,
       file_name,
       comments,
       magnet,
       created_at AS "created_at: jiff_sqlx::Timestamp"
FROM download_resolution WHERE download_id = ANY($1) ORDER BY resolution DESC;