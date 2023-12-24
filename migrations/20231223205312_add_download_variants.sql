ALTER TABLE episode_download
    RENAME TO download;
ALTER TABLE episode_download_resolution
    RENAME TO download_resolution;

CREATE TYPE download_variant AS ENUM ('batch', 'episode', 'movie');
ALTER TABLE download
    ALTER COLUMN episode DROP NOT NULL,
    ADD COLUMN variant download_variant NOT NULL DEFAULT 'episode';
ALTER TABLE download
    ALTER COLUMN variant DROP DEFAULT,
    ADD COLUMN start_index INTEGER,
    ADD COLUMN end_index   INTEGER,
    ADD CONSTRAINT valid_batch CHECK (variant != 'batch' OR (start_index IS NOT NULL AND end_index IS NOT NULL)),
    ADD CONSTRAINT valid_episode CHECK (variant != 'episode' OR episode IS NOT NULL);

CREATE INDEX download_updated_at_idx ON download (updated_at);
ALTER TABLE download_resolution
    ALTER COLUMN comments SET NOT NULL;

DROP INDEX IF EXISTS unique_episode;
CREATE UNIQUE INDEX unique_download ON download
    (variant, provider, title, episode, decimal, version, extra, start_index, end_index);