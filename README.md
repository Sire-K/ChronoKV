# ChronoKV

ChronoKV is a Rust workspace for an experimental key-value store and a RESP-compatible TCP server, paired with an optional semantic-cache API built on sentence embeddings.

The project is split into:
- **`kv-core`**: In-memory storage (`HashMap`), write-ahead logging (WAL), 4 KB page framing, snapshots, and crash recovery
- **`kv-net`**: Multithreaded RESP-compatible TCP server
- **`semantic-cache`**: FastAPI service performing embedding-based cache lookup and storage
- **`benchmarks`**: Comparison suite and visualization scripts for ChronoKV, Redis, the semantic cache, and `pgvector`

## Features

- Append-only write-ahead log (`chronokv.wal`) with CRC32 record validation
- Crash recovery through WAL replay on startup
- Fixed 4 KB page serialization and snapshot files (`chronokv.pages`)
- RESP bulk-string array and inline `PING`, `SET`, `GET`, and `DEL` commands
- Multithreaded TCP client handling
- Docker Compose deployment for ChronoKV, Semantic Cache, Redis, and `pgvector`
- Optional GPU/CPU embedding cache using `all-MiniLM-L6-v2` and cosine similarity

## Architecture

```text
Clients / applications
          |
          | HTTP (semantic cache) or RESP over TCP
          v
+----------------------+       +--------------------------+
| semantic-cache       |------>| kv-net:6379              |
| FastAPI + embeddings | RESP  | PING SET GET DEL         |
+----------------------+       +------------+-------------+
                                            |
                                            v
                               +--------------------------+
                               | kv-core                  |
                               | HashMap + WAL + pages    |
                               +------------+-------------+
                                            |
                                            v
                               /tmp/chronokv (or volume)
                               chronokv.wal / chronokv.pages

```

## Storage

`kv-core` maintains active key-value state in a `HashMap<String, Vec<u8>>`. Mutations are appended and flushed to the WAL before updating in-memory state. On startup, `Store::open` replays `chronokv.wal` to reconstruct state.

WAL records contain:

* CRC32 checksum
* Operation (`SET` or `DEL`)
* Key length and value length
* Key and value payload

The snapshot implementation writes fixed 4 KB pages to `chronokv.pages`. Replay stops cleanly if a corrupted or partial WAL record or snapshot page is encountered.

## Protocol

The TCP server (`0.0.0.0:6379`) accepts both RESP bulk-string arrays and a simple inline command format.

| Command | Example | Response |
| --- | --- | --- |
| `PING` | `PING` | `+PONG` |
| `SET` | `SET name chronokv` | `+OK` |
| `GET` | `GET name` | Bulk value or `$-1` |
| `DEL` | `DEL name` | `:1` when deleted, `:0` otherwise |

## Prerequisites

* **Rust** 1.85 or newer
* **Python** 3.10+ for the semantic cache and benchmark scripts
* **Docker** and **Docker Compose** for the container workflow
* **NVIDIA GPU (optional)**: The `semantic-cache` Docker service requests GPU access (`cuda`), falling back to CPU when CUDA is unavailable

## Quick Start

### Run the Rust Server

```bash
cargo run -p kv-net

```

ChronoKV listens on `0.0.0.0:6379` and creates its local data files under `/tmp/chronokv`:

```text
/tmp/chronokv/chronokv.wal
/tmp/chronokv/chronokv.pages

```

Test with RESP bulk-string arrays via `netcat`:

```bash
printf '*1\r\n$4\r\nPING\r\n' | nc 127.0.0.1 6379
printf '*3\r\n$3\r\nSET\r\n$4\r\nname\r\n$8\r\nChronoKV\r\n' | nc 127.0.0.1 6379
printf '*2\r\n$3\r\nGET\r\n$4\r\nname\r\n' | nc 127.0.0.1 6379

```

Inline commands are also accepted:

```bash
printf 'PING\n' | nc 127.0.0.1 6379
printf 'SET name ChronoKV\n' | nc 127.0.0.1 6379
printf 'GET name\n' | nc 127.0.0.1 6379
printf 'DEL name\n' | nc 127.0.0.1 6379

```

### Run with Docker Compose

From the repository root:

```bash
docker compose -f docker/docker-compose.yml up --build -d

```

Exposed services:

| Service | Address | Purpose |
| --- | --- | --- |
| **ChronoKV** | `127.0.0.1:6379` | RESP TCP server |
| **Semantic cache** | `127.0.0.1:8000` | FastAPI HTTP API |
| **Redis comparison** | `127.0.0.1:6380` | Benchmark reference only |
| **`pgvector`** | `127.0.0.1:5432` | Vector benchmark reference only |

> **Note:** `docker-compose.yml` mounts a named volume at `/data`, while `crates/kv-net/src/main.rs` currently initializes storage at `/tmp/chronokv`. Making the data directory configurable via environment variable is tracked under [Current Scope](https://www.google.com/search?q=%23current-scope).

## Semantic Cache API

`semantic-cache` loads `all-MiniLM-L6-v2`, stores serialized entries and embeddings in ChronoKV over RESP, and compares incoming query embeddings against cached entries using cosine similarity.

### Health Check

```bash
curl [http://127.0.0.1:8000/health](http://127.0.0.1:8000/health)

```

### Store a Response

```bash
curl -X POST [http://127.0.0.1:8000/cache/store](http://127.0.0.1:8000/cache/store) \
  -H 'Content-Type: application/json' \
  -d '{"query":"How do I recover a database after a crash?","response":"Replay the WAL and verify checksums."}'

```

### Look Up a Response

```bash
curl -X POST [http://127.0.0.1:8000/cache/lookup](http://127.0.0.1:8000/cache/lookup) \
  -H 'Content-Type: application/json' \
  -d '{"query":"What are the steps for crash recovery in a DB?","threshold":0.78}'

```

A cache hit requires the cosine similarity score to meet the per-request `threshold` or the configured `SIMILARITY_THRESHOLD`.

## Configuration

### Semantic Cache Environment Variables

| Variable | Default | Description |
| --- | --- | --- |
| `CHRONOKV_HOST` | `127.0.0.1` | ChronoKV hostname (`chronokv` in Docker Compose) |
| `CHRONOKV_PORT` | `6379` | ChronoKV RESP TCP port |
| `EMBEDDING_MODEL` | `all-MiniLM-L6-v2` | Sentence Transformers model name |
| `SIMILARITY_THRESHOLD` | `0.85` | Default cosine-similarity threshold (`0.78` in Docker Compose) |

## Development

Build and test the Rust workspace:

```bash
cargo build --workspace
cargo test --workspace

```

Run the server locally without Docker:

```bash
cargo run -p kv-net

```

The Rust test suite covers:

* WAL `SET`/`DEL` replay
* 4 KB page round trips and malformed headers
* Recovery stopping cleanly at corrupted or partial records
* Store recovery and snapshot-page creation

## Benchmarks & Profiling

`benchmarks/benchmark_compare.py` compares:

* ChronoKV RESP round trips (`127.0.0.1:6379`)
* Redis RESP round trips (`127.0.0.1:6380`)
* ChronoKV semantic-cache lookup latency (`POST /cache/lookup`, including live text embedding)
* `pgvector` top-1 cosine search (`500` precomputed 384-dim vectors)

### Local Benchmark Results

| Workload | Throughput | p50 | p95 | p99 | Notes |
| --- | --- | --- | --- | --- | --- |
| **ChronoKV RESP (`SET`/`GET`)** | **27,335.7 ops/s** | **0.067 ms** | 0.105 ms | 0.320 ms | Single-connection sequential loop |
| **Redis RESP (`SET`/`GET`)** | 4,491.4 ops/s | 0.073 ms | **0.087 ms** | **0.102 ms** | Containerized reference (`:6380`) |
| **ChronoKV Semantic Cache** | — | 8.137 ms | 14.094 ms | 15.355 ms | Full HTTP + `all-MiniLM-L6-v2` inference + KV |
| **`pgvector` Top-1 Cosine** | — | **0.209 ms** | **0.446 ms** | **1.748 ms** | SQL vector distance on 500 precomputed rows |

### Reproduce Benchmarks & Plots

With the Docker Compose stack running:

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install numpy psycopg[binary] matplotlib
python benchmarks/benchmark_compare.py
python benchmarks/plot_p99.py

```

*Benchmark outputs and plotted charts are informational results from local experiments, not a production performance guarantee.*

### CPU Flamegraph

A CPU flamegraph for `kv-net` captured under load with `perf` and `inferno-flamegraph` is stored at `benchmarks/flamegraph.svg`:

## Repository Layout

```text
ChronoKV/
├── Cargo.toml
├── crates/
│   ├── kv-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── page.rs
│   │       ├── store.rs
│   │       └── wal.rs
│   └── kv-net/
│       └── src/
│           ├── main.rs
│           └── resp.rs
:├── semantic-cache/
│   ├── app/
│   │   └── main.py
│   ├── Dockerfile
│   └── requirements.txt
├── docker/
│   └── docker-compose.yml
├── benchmarks/
│   ├── benchmark_compare.py
│   ├── plot_p99.py
│   ├── flamegraph.svg
│   └── p99_latency.png
└── README.md

```

## Current Scope

ChronoKV is primarily an implementation-and-learning project rather than a drop-in replacement for Redis or a dedicated vector database. Key areas still incomplete include:

* Configurable storage path and bind address in `kv-net` so Docker mounts `/data` directly instead of `/tmp/chronokv`
* Snapshot loading/reuse on startup (`chronokv.pages`) to bound WAL replay time
* Cross-page entries for payloads larger than a single 4 KB page
* Expanded Redis-compatible command set (`MGET`, `MSET`, `EXPIRE`, `TTL`, `INFO`)

## License

This project is licensed under the Apache License 2.0.
