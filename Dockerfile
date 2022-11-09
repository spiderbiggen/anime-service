# syntax=docker/dockerfile:1

FROM rust:1.65 as Builder

WORKDIR /app

COPY ./Cargo.toml ./

RUN cargo fetch

COPY ./src ./src
COPY ./consume_api ./consume_api
COPY ./kitsu ./kitsu
COPY ./nyaa ./nyaa
COPY ./Rocket.toml ./Rocket.toml

RUN cargo build --release

FROM gcr.io/distroless/static as Application

WORKDIR /opt

COPY --from=Builder /anime-service ./anime-service
COPY --from=Builder /app/Rocket.toml ./Rocket.toml

EXPOSE 8000
ENTRYPOINT ["/opt/anime-service"]