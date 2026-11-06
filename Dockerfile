# syntax=docker/dockerfile:1

# Multistage build: `rpt` and `rpt-render` linked against glibc + OpenSSL.
#
# The SQL Server driver (`tiberius` / native-tls) needs OpenSSL on Linux. A fully static `scratch`
# musl image cannot satisfy that without a large vendored-OpenSSL setup; this image ships a small
# runtime with CA certificates and `libssl3` instead.
#
# Pinned to the workspace MSRV; with the committed Cargo.lock the build is reproducible.
FROM rust:1.92-slim-bookworm AS builder
WORKDIR /src

# `libsqlite3-sys` compiles C; `openssl-sys` (via native-tls / MSSQL) links OpenSSL.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

# Cache mounts speed rebuilds; binaries land in /out for the runtime stage.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p rpt-cli -p rpt-render-cli \
    && mkdir -p /out \
    && cp target/release/rpt target/release/rpt-render /out/

FROM debian:bookworm-slim AS runtime

# TLS to DB servers (postgres with TLS, `mssql://` with native-tls).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --system --no-create-home --shell /usr/sbin/nologin rpt

COPY --from=builder /out/rpt /out/rpt-render /usr/local/bin/

COPY --from=builder /src/LICENSE /src/THIRD-PARTY-NOTICES.md /usr/local/share/licenses/rpt-rs/

LABEL org.opencontainers.image.source="https://github.com/MrSrsen/rpt-rs" \
      org.opencontainers.image.licenses="MPL-2.0"

ENV PATH=/usr/local/bin
WORKDIR /data
USER 10001:10001

# Mount reports here, e.g. `docker run --rm -v "$PWD:/data" IMAGE rpt-render report.rpt --db -o out.pdf`
# Live DB URL must come from the environment (RPT_DB_URL or DATABASE_URL).
CMD ["rpt", "--help"]
