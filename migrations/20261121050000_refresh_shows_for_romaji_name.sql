-- Show summaries now carry `romaji_name`. Rows stored before that lack it, so mark them stale: they
-- keep being served as they are, and are re-fetched the next time a release for the show arrives
-- (see `show_ids_to_refresh`).
UPDATE show
SET fetched_at = now() - INTERVAL '31 days';
