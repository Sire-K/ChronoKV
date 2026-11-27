import json
import socket
import time
import urllib.request
import numpy as np
import psycopg

def resp_cmd(sock: socket.socket, rfile, *parts: str) -> bytes:
    chunks = [f"*{len(parts)}\r\n".encode()]
    for p in parts:
        b = p.encode()
        chunks.append(f"${len(b)}\r\n".encode() + b + b"\r\n")
    sock.sendall(b"".join(chunks))

    header = rfile.readline()
    if not header:
        raise ConnectionError("EOF from RESP server")
    if not header.endswith(b"\r\n"):
        raise ValueError(f"Malformed RESP line: {header!r}")

    prefix = header[:1]
    payload = header[1:-2]
    if prefix in (b"+", b":", b"-"):
        return payload
    if prefix == b"$":
        n = int(payload)
        if n < 0:
            return b""
        data = rfile.read(n + 2)
        if len(data) < n + 2:
            raise ConnectionError("EOF reading bulk payload")
        return data[:n]
    return payload


def bench_resp(host: str, port: int, n_ops: int = 1000) -> dict:
    lat_ms = []
    with socket.create_connection((host, port), timeout=10) as s:
        rfile = s.makefile("rb")
        for i in range(50):
            resp_cmd(s, rfile, "SET", f"warm:{i}", "val")
            resp_cmd(s, rfile, "GET", f"warm:{i}")

        t0 = time.perf_counter()
        for i in range(n_ops):
            k = f"bench:key:{i}"
            v = f"payload-value-{i}-" + ("x" * 64)
            s0 = time.perf_counter()
            resp_cmd(s, rfile, "SET", k, v)
            resp_cmd(s, rfile, "GET", k)
            lat_ms.append((time.perf_counter() - s0) * 1000.0)
        elapsed = time.perf_counter() - t0

    arr = np.array(lat_ms)
    return {
        "ops_per_sec": round((n_ops * 2) / elapsed, 1),
        "p50_ms": round(float(np.percentile(arr, 50)), 3),
        "p95_ms": round(float(np.percentile(arr, 95)), 3),
        "p99_ms": round(float(np.percentile(arr, 99)), 3),
    }

def bench_semantic_cache(n_queries: int = 50) -> dict:
    seed_queries = [
        ("How do I recover a database after a crash?", "Replay WAL and verify checksums."),
        ("How does B-Tree page splitting work?", "Split full node and promote median key."),
        ("Why use Write-Ahead Logging before checkpoints?", "Guarantees durability and atomic recovery."),
        ("How do I index high-dimensional embeddings fast?", "Use approximate nearest neighbors or HNSW."),
    ]
    for q, r in seed_queries:
        req = urllib.request.Request(
            "http://127.0.0.1:8000/cache/store",
            data=json.dumps({"query": q, "response": r}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=15):
            pass

    probe = "What are the steps for crash recovery in a DB?"
    lat_ms = []
    for _ in range(n_queries):
        s0 = time.perf_counter()
        req = urllib.request.Request(
            "http://127.0.0.1:8000/cache/lookup",
            data=json.dumps({"query": probe, "threshold": 0.70}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=15) as resp:
            resp.read()
        lat_ms.append((time.perf_counter() - s0) * 1000.0)

    arr = np.array(lat_ms)
    return {
        "p50_ms": round(float(np.percentile(arr, 50)), 3),
        "p95_ms": round(float(np.percentile(arr, 95)), 3),
        "p99_ms": round(float(np.percentile(arr, 99)), 3),
    }


def bench_pgvector(n_queries: int = 50, n_rows: int = 500) -> dict:
    rng = np.random.default_rng(42)
    vecs = rng.standard_normal((n_rows, 384), dtype=np.float32)
    vecs /= np.linalg.norm(vecs, axis=1, keepdims=True)
    qvec = vecs[0].tolist()

    dsn = "postgresql://chronokv:chronokv@127.0.0.1:5432/chronokv"
    with psycopg.connect(dsn, autocommit=True) as conn:
        conn.execute("CREATE EXTENSION IF NOT EXISTS vector")
        conn.execute("DROP TABLE IF EXISTS bench_items")
        conn.execute("CREATE TABLE bench_items (id serial PRIMARY KEY, emb vector(384))")
        with conn.cursor() as cur:
            for row in vecs:
                vec_str = "[" + ",".join(f"{x:.6f}" for x in row.tolist()) + "]"
                cur.execute("INSERT INTO bench_items (emb) VALUES (%s::vector)", (vec_str,))

        q_str = "[" + ",".join(f"{x:.6f}" for x in qvec) + "]"
        lat_ms = []
        for _ in range(n_queries):
            s0 = time.perf_counter()
            conn.execute(
                "SELECT id, emb <=> %s::vector AS dist FROM bench_items ORDER BY dist ASC LIMIT 1",
                (q_str,),
            ).fetchone()
            lat_ms.append((time.perf_counter() - s0) * 1000.0)

    arr = np.array(lat_ms)
    return {
        "p50_ms": round(float(np.percentile(arr, 50)), 3),
        "p95_ms": round(float(np.percentile(arr, 95)), 3),
        "p99_ms": round(float(np.percentile(arr, 99)), 3),
    }


if __name__ == "__main__":
    print("ChronoKV RESP (127.0.0.1:6379):", bench_resp("127.0.0.1", 6379))
    print("Redis RESP    (127.0.0.1:6380):", bench_resp("127.0.0.1", 6380))
    print("ChronoKV Semantic Cache (/cache/lookup):", bench_semantic_cache())
    print("pgvector top-1 cosine (500 rows):", bench_pgvector())
