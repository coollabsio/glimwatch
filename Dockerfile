# syntax=docker/dockerfile:1.7
FROM oven/bun:1 AS web
WORKDIR /src/apps/web
COPY apps/web/package.json apps/web/bun.lock ./
RUN bun install --frozen-lockfile
COPY apps/web/ ./
RUN bun run build

FROM rust:1-bookworm AS server
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/server/ apps/server/
COPY --from=web /src/apps/web/dist/ apps/web/dist/
RUN cargo build --locked --release -p haruvo-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 65532 --user-group --create-home --home-dir /var/lib/haruvo haruvo \
    && mkdir -p /var/lib/haruvo \
    && chown -R 65532:65532 /var/lib/haruvo
COPY --from=server /src/target/release/haruvo /haruvo
ENV HARUVO_ENV=production \
    HARUVO_BIND=0.0.0.0 \
    HARUVO_PORT=8080 \
    HARUVO_DATABASE_PATH=/var/lib/haruvo/haruvo.sqlite \
    HARUVO_PUBLIC_ORIGIN=http://127.0.0.1:8080
USER 65532:65532
VOLUME ["/var/lib/haruvo"]
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=10 CMD ["/haruvo", "healthcheck"]
ENTRYPOINT ["/haruvo"]
CMD ["serve"]
