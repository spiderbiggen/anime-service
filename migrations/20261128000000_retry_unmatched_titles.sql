-- The resolver now retries a search without the season suffix (such as ` S2`). Age the automatic
-- lookups that found nothing, so they are retried the next time a release for the title arrives
-- (see `unresolved_titles`) instead of up to a week later.
UPDATE title_match
SET checked_at = now() - INTERVAL '8 days'
WHERE tmdb_id IS NULL
  AND source = 'auto';
