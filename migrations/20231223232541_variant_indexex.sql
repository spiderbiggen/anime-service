DROP INDEX unique_download;
CREATE UNIQUE INDEX download_batch_variant_unique_idx ON download (provider, title, start_index, end_index) WHERE variant = 'batch';
CREATE UNIQUE INDEX download_episode_variant_unique_idx ON download (provider, title, episode, decimal, version, extra) WHERE variant = 'episode';
CREATE UNIQUE INDEX download_movie_variant_unique_idx ON download (provider, title) WHERE variant = 'movie';