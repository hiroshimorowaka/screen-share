# syntax=docker/dockerfile:1

FROM rust:1-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
    && cargo install cargo-leptos --locked

WORKDIR /app
COPY . .

# Cache Cargo's registry/git downloads and the target/ build directory
# across builds via BuildKit cache mounts. These persist independently of
# Docker's layer cache — including on Fly's remote builder between separate
# `fly deploy` runs — so cargo only recompiles what Cargo.lock or src/*.rs
# actually changed instead of the whole ~250-crate dependency graph every
# time. target/ (and target/site, which lives under it) has to be copied
# out to a normal path before the mount unmounts, since anything left
# inside a cache mount never makes it into the resulting image layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo leptos build --release \
    && cp target/release/screen-share-server /app/screen_share.out \
    && cp -r target/site /app/site.out

# --- runtime image: only the compiled binary + generated site assets ---
FROM debian:bookworm-slim AS runtime

# coturn: the self-hosted TURN relay, run as a second process alongside the
# app inside the same Fly Machine — see docker-entrypoint.sh. Only actually
# starts if TURN_SECRET/TURN_EXTERNAL_IP are set; a deployment without them
# just runs the app on its own, STUN-only.
#
# coturn is taken unpinned from bookworm (pinning an exact apt version
# breaks on the next Debian point release). The image only picks up coturn
# security fixes on a rebuild, so this needs a scheduled rebuild+redeploy
# (ops task, tracked in docs/decisions/0008-security-hardening.md) — the
# `--denied-peer-ip` hardening in docker-entrypoint.sh is what bounds the
# blast radius until then.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates coturn \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/screen_share.out ./screen_share
COPY --from=builder /app/site.out ./site
COPY docker-entrypoint.sh ./docker-entrypoint.sh

ENV LEPTOS_OUTPUT_NAME=screen_share
ENV LEPTOS_SITE_ROOT=site
ENV LEPTOS_SITE_PKG_DIR=pkg
ENV LEPTOS_ENV=PROD
# Fly.io (unlike Render) doesn't inject a dynamic $PORT — you pick a fixed
# port and declare it once in fly.toml's internal_port. Keep these in sync.
ENV LEPTOS_SITE_ADDR=0.0.0.0:8080

EXPOSE 8080
# STUN/TURN control port, plus the relay port range coturn allocates from —
# must match TURN_MIN_PORT/TURN_MAX_PORT and fly.toml's UDP services.
EXPOSE 3478/udp
EXPOSE 49160-49300/udp

CMD ["./docker-entrypoint.sh"]
