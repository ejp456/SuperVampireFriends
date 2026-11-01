# ==============================================================================
# Stage 1: Build Bevy WebAssembly Client & Rust WebSocket Co-op Relay Server
# ==============================================================================
FROM rust:1-bookworm AS builder

WORKDIR /app

COPY .cargo ./.cargo
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets
COPY web ./web

# 1. Build optimized WebAssembly game client & JS bindings (uses prebuilt web/pkg
#    when present in the build context, or compiles from source on clean clones / CI)
RUN if [ ! -f web/pkg/vampire_noir_rpg_bg.wasm ]; then \
        rustup target add wasm32-unknown-unknown \
        && curl -fsSL https://github.com/rustwasm/wasm-bindgen/releases/download/0.2.130/wasm-bindgen-0.2.130-x86_64-unknown-linux-musl.tar.gz \
           | tar -xz -C /usr/local/bin --strip-components=1 wasm-bindgen-0.2.130-x86_64-unknown-linux-musl/wasm-bindgen \
        && cargo build --release --target wasm32-unknown-unknown --bin vampire_noir_rpg \
        && mkdir -p web/pkg \
        && wasm-bindgen --out-dir web/pkg --target web --no-typescript \
           --remove-name-section --remove-producers-section \
           target/wasm32-unknown-unknown/release/vampire_noir_rpg.wasm; \
    fi \
    && if [ ! -f web/pkg/vampire_noir_rpg_bg.wasm.gz ]; then \
        gzip -9 -k -f web/pkg/vampire_noir_rpg_bg.wasm web/pkg/vampire_noir_rpg.js; \
    fi

# 2. Compile zero-dependency HTTP + WebSocket Co-op Relay server directly with rustc
#    (avoids pulling Linux winit display-server features from Cargo.toml's Bevy graph)
RUN rustc -O --edition 2021 src/bin/web_server.rs -o /app/web_server_bin

# ==============================================================================
# Stage 2: Minimal Production Runtime Image (Cloud Run / Railway / Fly.io ready)
# ==============================================================================
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/web_server_bin /app/web_server
COPY --from=builder /app/web /app/web
COPY --from=builder /app/assets /app/assets

ENV PORT=8080
EXPOSE 8080

CMD ["/app/web_server"]
