CREATE TABLE public.movie_download
(
    id         uuid PRIMARY KEY         NOT NULL DEFAULT uuid_generate_v4(),
    provider   VARCHAR(255)             NOT NULL DEFAULT 'SubsPlease',
    title      VARCHAR                  NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
CREATE INDEX movie_download_title ON movie_download USING btree (title);
CREATE UNIQUE INDEX unique_movie ON movie_download USING btree (provider, title);

CREATE TABLE public.movie_download_resolution
(
    movie_download_id uuid                     NOT NULL,
    resolution        SMALLINT                 NOT NULL,
    torrent           VARCHAR(1024)            NOT NULL,
    file_name         TEXT                     NOT NULL,
    comments          TEXT,
    magnet            TEXT,
    created_at        TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (movie_download_id, resolution),
    FOREIGN KEY (movie_download_id) REFERENCES public.movie_download (id)
        MATCH SIMPLE ON UPDATE NO ACTION ON DELETE CASCADE
);

CREATE TABLE public.batch_download
(
    id          uuid PRIMARY KEY         NOT NULL DEFAULT uuid_generate_v4(),
    provider    VARCHAR(255)             NOT NULL DEFAULT 'SubsPlease',
    title       VARCHAR                  NOT NULL,
    start_index INTEGER                  NOT NULL,
    end_index   INTEGER                  NOT NULL,
    created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
CREATE INDEX batch_download_title ON batch_download USING btree (title);
CREATE UNIQUE INDEX unique_batch ON batch_download USING btree (provider, title, start_index, end_index);

CREATE TABLE public.batch_download_resolution
(
    batch_download_id uuid                     NOT NULL,
    resolution        SMALLINT                 NOT NULL,
    torrent           VARCHAR(1024)            NOT NULL,
    file_name         TEXT                     NOT NULL,
    comments          TEXT,
    magnet            TEXT,
    created_at        TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (batch_download_id, resolution),
    FOREIGN KEY (batch_download_id) REFERENCES public.batch_download (id)
        MATCH SIMPLE ON UPDATE NO ACTION ON DELETE CASCADE
);