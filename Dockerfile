# syntax=docker/dockerfile:1.7
#
# Builds the authoritative game server only. The desktop client is shipped
# separately via GitHub Releases (see .github/workflows/release.yml).

FROM rust:1.96-bookworm AS builder
WORKDIR /app

# Cache dependency compilation separately from source changes.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY tools ./tools
RUN cargo build --release -p openmmo-server --features postgres,redis \
    && cp target/release/openmmo-server /usr/local/bin/openmmo-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/local/bin/openmmo-server /usr/local/bin/openmmo-server
COPY content ./content

ENV OPENMMO_CONTENT=/app/content \
    RUST_LOG=info

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
    CMD curl -fsS "http://127.0.0.1:${PORT:-8080}/health" || exit 1

CMD ["openmmo-server"]
