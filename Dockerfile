# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1.65 as builder

RUN apt-get update && apt-get install -qq g++-aarch64-linux-gnu libc6-dev-arm64-cross
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
ENV CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
ENV CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++

ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
  "linux/arm64") echo aarch64-unknown-linux-gnu > /rust_target.txt ;; \
  "linux/amd64") echo x86_64-unknown-linux-gnu > /rust_target.txt ;; \
  *) exit 1 ;; \
esac

RUN rustup target add $(cat /rust_target.txt)

COPY . ./
RUN cargo build --release --target $(cat /rust_target.txt)
RUN cp ./target/$(cat /rust_target.txt)/release/anime-service /anime-service

FROM gcr.io/distroless/cc as application

COPY --from=builder /anime-service /

EXPOSE 8000
ENTRYPOINT ["./anime-service"]