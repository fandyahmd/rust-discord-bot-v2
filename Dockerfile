# syntax=docker/dockerfile:1.7

# ── Build ────────────────────────────────────────────────────────────────
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# rustls dipakai (tanpa OpenSSL), jadi cuma butuh pkg-config.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY lavalink-rs-patched ./lavalink-rs-patched
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked \
    && cp target/release/rust /usr/local/bin/rust

# ── Runtime ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 bot

COPY --from=builder /usr/local/bin/rust /usr/local/bin/rust

USER bot
WORKDIR /home/bot

ENV RUST_LOG=info,lavalink_rs=info \
    PORT=8080

# Health endpoint for PaaS platforms (Render/Koyeb web service) so the
# container is not considered idle and put to sleep.
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=40s --retries=3 \
    CMD ["sh", "-c", "exec 3<>/dev/tcp/127.0.0.1/8080 && printf 'GET /healthz HTTP/1.1\\r\\nHost: localhost\\r\\nConnection: close\\r\\n\\r\\n' >&3 && head -c 100 <&3 | grep -q '200 OK'"]

CMD ["rust"]
