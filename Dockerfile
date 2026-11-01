# ==============================================================================
# Stage 1: Build Bevy WebAssembly Client & Rust WebSocket Co-op Relay Server
# ==============================================================================
FROM rust:1.85-bookworm AS builder

WORKDIR /app

RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.130 --locked

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets
COPY web ./web

# 1. Build optimized WebAssembly game client & JS bindings
RUN cargo build --release --target wasm32-unknown-unknown --bin vampire_noir_rpg \
    && mkdir -p web/pkg \
    && wasm-bindgen --out-dir web/pkg --target web --no-typescript \
       target/wasm32-unknown-unknown/release/vampire_noir_rpg.wasm

# 2. Build zero-dependency HTTP + WebSocket Co-op Relay server
RUN cargo build --release --bin web_server

# ==============================================================================
# Stage 2: Minimal Production Runtime Image (Cloud Run / Railway / Fly.io ready)
# ==============================================================================
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/web_server /app/web_server
COPY --from=builder /app/web /app/web
COPY --from=builder /app/assets /app/assets

ENV PORT=8080
EXPOSE 8080

CMD ["/app/web_server"]
