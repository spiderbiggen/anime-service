ALTER TABLE download RENAME CONSTRAINT episode_download_pkey TO download_pkey;
ALTER INDEX IF EXISTS episode_download_title RENAME TO download_title;
ALTER TABLE download_resolution RENAME CONSTRAINT episode_download_resolution_pkey TO download_resolution_pkey;
ALTER TABLE download_resolution RENAME CONSTRAINT episode_download_resolution_episode_download_id_fkey TO download_resolution_episode_download_id_fkey;
ALTER TABLE download_resolution RENAME COLUMN episode_download_id TO download_id;