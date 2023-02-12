ALTER TABLE episode_download
    ADD COLUMN updated_at TIMESTAMPTZ;

UPDATE episode_download ed
SET updated_at = (SELECT max(edr.created_at)
                  from episode_download_resolution edr
                  WHERE edr.episode_download_id = ed.id
                  group by edr.episode_download_id)
WHERE true;

ALTER TABLE episode_download
    ALTER COLUMN created_at SET DEFAULT now(),
    ALTER COLUMN updated_at SET DEFAULT now(),
    ALTER COLUMN updated_at SET NOT NULL;

