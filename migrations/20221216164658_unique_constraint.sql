-- Add migration script here
ALTER TABLE episode_download
    DROP CONSTRAINT IF EXISTS unique_episode;
DROP INDEX IF EXISTS episode_download.unique_episode ;
DROP INDEX IF EXISTS unique_episode ;

DELETE FROM episode_download
WHERE id IN (
    SELECT id FROM episode_download
    EXCEPT SELECT (array_agg(id))[1] FROM episode_download
    GROUP BY provider, title, episode, decimal, version
);

CREATE UNIQUE INDEX unique_episode on episode_download
    (provider, title, COALESCE(episode, 0), COALESCE(decimal, 0), COALESCE(version, 0));
