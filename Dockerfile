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

ENV RUST_LOG=info,lavalink_rs=info

# Bot tidak membuka port; jalankan sebagai worker/background service.
CMD ["rust"]
