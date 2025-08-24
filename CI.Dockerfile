FROM gcr.io/distroless/cc-debian12 AS application

ARG TARGETPLATFORM

COPY --chmod=0755 "exec/$TARGETPLATFORM" /anime-service

EXPOSE 8000
ENTRYPOINT ["/anime-service"]
