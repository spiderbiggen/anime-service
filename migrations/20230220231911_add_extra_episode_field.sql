ALTER TABLE episode_download
    ADD COLUMN extra TEXT;

DROP INDEX IF EXISTS unique_episode ;

CREATE UNIQUE INDEX unique_episode on episode_download
    (provider, title, COALESCE(episode, 0), COALESCE(decimal, 0), COALESCE(version, 0), COALESCE(extra, ''));