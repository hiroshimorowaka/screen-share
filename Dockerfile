# syntax=docker/dockerfile:1

FROM rust:1-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
    && cargo install cargo-leptos --locked

WORKDIR /app
COPY . .

RUN cargo leptos build --release

# --- runtime image: only the compiled binary + generated site assets ---
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/screen_share ./screen_share
COPY --from=builder /app/target/site ./site

ENV LEPTOS_OUTPUT_NAME=screen_share
ENV LEPTOS_SITE_ROOT=site
ENV LEPTOS_SITE_PKG_DIR=pkg
ENV LEPTOS_ENV=PROD
# Fly.io (unlike Render) doesn't inject a dynamic $PORT — you pick a fixed
# port and declare it once in fly.toml's internal_port. Keep these in sync.
ENV LEPTOS_SITE_ADDR=0.0.0.0:8080

EXPOSE 8080

CMD ["./screen_share"]
