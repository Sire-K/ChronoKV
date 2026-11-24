# ChronoKV

A first-principles key-value storage engine and TCP server built in Rust (`kv-core` and `kv-net`), paired with an optional GPU-accelerated semantic cache service (`semantic-cache`). Built hands-on from the ground up with fixed 4 KB pages, Write-Ahead Logging (WAL), crash recovery, and RESP-style wire commands.

## Architecture Overview

ChronoKV separates deterministic storage and networking from AI similarity lookup so exact-key operations stay fast and easy to debug:

1. **Storage & WAL Core (`crates/kv-core`)**
   - **4 KB Pages:** Fixed 4096-byte page framing and serialization primitives.
   - **Write-Ahead Log (WAL):** Append-only binary log records (`length`, `crc32` checksum, operation, key/value payload) written before mutating in-memory state.
   - **Crash Recovery:** Replays WAL records on startup, validates checksums, and stops cleanly at partial/corrupt tail records after abrupt kills (`kill -9`).
2. **TCP & RESP Server (`crates/kv-net`)**
   - Listens over TCP, parses commands (`PING`, `SET`, `GET`, `DEL`), and routes mutations through `kv-core`.
3. **Semantic Cache Service (`semantic-cache/`)**
   - Optional container service using PyTorch / embeddings on an NVIDIA GPU to check semantic similarity before falling back to exact lookups in ChronoKV.

## Repository Layout

```text
chronokv/
├── Cargo.toml
├── README.md
├── crates/
│   ├── kv-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── page.rs
│   │       ├── wal.rs
│   │       └── store.rs
│   └── kv-net/
│       └── src/
│           └── main.rs
|           └── resp.rs
├── semantic-cache/
│   ├── Dockerfile
│   ├── requirements.txt
│   └── app/
│       └── main.py
├── docker/
│   └── docker-compose.yml
└── docs/
    └── architecture.md
