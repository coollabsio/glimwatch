# syntax=docker/dockerfile:1.7
# UI assets are architecture-independent. Build them on BUILDPLATFORM so
# a cross-build does not emulate Bun. CI builds amd64 and arm64 on native
# runners, where this stage is native too.
FROM --platform=$BUILDPLATFORM oven/bun:1 AS web
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
RUN cargo build --locked --release -p statixo-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 65532 --user-group --create-home --home-dir /var/lib/statixo statixo \
    && mkdir -p /var/lib/statixo \
    && chown -R 65532:65532 /var/lib/statixo
COPY --from=server /src/target/release/statixo /statixo
ENV STATIXO_ENV=production \
    STATIXO_BIND=0.0.0.0 \
    STATIXO_PORT=8080 \
    STATIXO_DATABASE_PATH=/var/lib/statixo/statixo.sqlite \
    STATIXO_PUBLIC_ORIGIN=http://127.0.0.1:8080
USER 65532:65532
VOLUME ["/var/lib/statixo"]
EXPOSE 8080
HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=10 CMD ["/statixo", "healthcheck"]
ENTRYPOINT ["/statixo"]
CMD ["serve"]
