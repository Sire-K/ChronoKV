FROM rust:1.82-slim AS builder

WORKDIR /usr/src/chronokv

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release -p kv-net

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/chronokv/target/release/kv-net /usr/local/bin/kv-net

WORKDIR /data
EXPOSE 6379

CMD ["kv-net"]
