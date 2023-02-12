ALTER TABLE episode_download
    ADD COLUMN updated_at TIMESTAMPTZ;

UPDATE episode_download
SET updated_at = created_at;

ALTER TABLE episode_download
    ALTER COLUMN created_at SET DEFAULT now(),
    ALTER COLUMN updated_at SET DEFAULT now(),
    ALTER COLUMN updated_at SET NOT NULL;

