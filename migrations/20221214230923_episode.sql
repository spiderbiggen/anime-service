-- Add migration script here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS episode_download
(
    id         UUID PRIMARY KEY      DEFAULT uuid_generate_v4(),
    provider   VARCHAR(255) NOT NULL DEFAULT 'SubsPlease',
    title      TEXT         NOT NULL,
    episode    INTEGER,
    decimal    INTEGER,
    version    INTEGER,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT unique_episode UNIQUE (provider, title, episode, decimal, version)
);

CREATE INDEX episode_download_title ON episode_download (title);


CREATE TABLE IF NOT EXISTS episode_download_resolution
(
    episode_download_id UUID          NOT NULL,
    resolution          VARCHAR(16)    NOT NULL,
    torrent             VARCHAR(1024) NOT NULL,
    file_name           TEXT          NOT NULL,
    comments            VARCHAR(1024),
    magnet              TEXT,
    created_at          TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (episode_download_id, resolution),
    FOREIGN KEY (episode_download_id) REFERENCES episode_download (id) ON DELETE CASCADE
);