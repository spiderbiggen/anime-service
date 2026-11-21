SELECT id,
       provider,
       title,
       episode,
       decimal,
       version,
       created_at AS "created_at: jiff_sqlx::Timestamp",
       updated_at AS "updated_at: jiff_sqlx::Timestamp",
       extra,
       variant as "variant: Variant",
       start_index,
       end_index
FROM download
WHERE ($1::download_variant IS NULL OR variant = $1::download_variant)
  AND (title ILIKE COALESCE($2, '') || '%')
ORDER BY updated_at DESC
LIMIT 25;