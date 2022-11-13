# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1.65 as Builder

WORKDIR /app

RUN apt-get update && apt-get install -qq g++-aarch64-linux-gnu gcc-aarch64-linux-gnu

COPY ./Cargo.toml ./
COPY ./src ./src
COPY ./consume_api ./consume_api
COPY ./kitsu ./kitsu
COPY ./nyaa ./nyaa
COPY ./Rocket.toml ./Rocket.toml

ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
"linux/arm64") echo aarch64-unknown-linux-gnu > /rust_target.txt ;; \
"linux/amd64") echo x86_64-unknown-linux-gnu > /rust_target.txt ;; \
*) exit 1 ;; \
esac

RUN rustup target add $(cat /rust_target.txt)
RUN cargo build --release --target $(cat /rust_target.txt)
RUN cp target/$(cat /rust_target.txt)/release/anime-service ./

FROM gcr.io/distroless/static as Application

WORKDIR /opt

COPY --from=Builder /app/anime-service ./anime-service
COPY --from=Builder /app/Rocket.toml ./Rocket.toml

EXPOSE 8000
ENTRYPOINT ["/opt/anime-service"]