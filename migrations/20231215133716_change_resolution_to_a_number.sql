ALTER TABLE episode_download_resolution
    RENAME COLUMN resolution TO resolution_text;
ALTER TABLE episode_download_resolution
    ADD COLUMN resolution SMALLINT;

UPDATE episode_download_resolution
SET resolution = RTRIM(resolution_text::VARCHAR, 'p')::SMALLINT
WHERE TRUE;

ALTER TABLE episode_download_resolution
    ALTER COLUMN resolution SET NOT NULL,
    DROP COLUMN resolution_text;
ALTER TABLE episode_download
    ALTER COLUMN title TYPE VARCHAR,
    ALTER COLUMN episode SET NOT NULL;
ALTER TABLE episode_download_resolution
    DROP CONSTRAINT IF EXISTS episode_download_resolution_pk,
    ADD PRIMARY KEY (episode_download_id, resolution);

DROP INDEX IF EXISTS unique_episode;
CREATE UNIQUE INDEX unique_episode ON episode_download
    (provider, title, episode, COALESCE(decimal, 0), COALESCE(version, 0), COALESCE(extra, ''));