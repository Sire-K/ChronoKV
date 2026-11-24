import json
import os
import socket
from typing import Optional

import numpy as np
import torch
from fastapi import FastAPI
from pydantic import BaseModel
from sentence_transformers import SentenceTransformer

CHRONOKV_HOST = os.getenv("CHRONOKV_HOST", "127.0.0.1")
CHRONOKV_PORT = int(os.getenv("CHRONOKV_PORT", "6379"))
MODEL_NAME = os.getenv("EMBEDDING_MODEL", "all-MiniLM-L6-v2")
SIMILARITY_THRESHOLD = float(os.getenv("SIMILARITY_THRESHOLD", "0.85"))
INDEX_KEY = "semantic:index:ids"

DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
model = SentenceTransformer(MODEL_NAME, device=DEVICE)
app = FastAPI(title="ChronoKV Semantic Cache")


class QueryRequest(BaseModel):
    query: str
    response: Optional[str] = None
    threshold: Optional[float] = None


def kv_command(*parts: str) -> str:
    payload = (
        f"*{len(parts)}\r\n"
        + "".join(f"${len(p.encode('utf-8'))}\r\n{p}\r\n" for p in parts)
    ).encode("utf-8")
    with socket.create_connection((CHRONOKV_HOST, CHRONOKV_PORT), timeout=5) as sock:
        sock.sendall(payload)
        raw = sock.recv(65536).decode("utf-8", errors="replace").strip()

    if raw.startswith("+"):
        return raw[1:]
    if raw.startswith("$"):
        lines = raw.splitlines()
        if len(lines) >= 2 and lines[0] != "$-1":
            return lines[1]
        return ""
    return raw


def embed_text(text: str) -> np.ndarray:
    vec = model.encode([text], normalize_embeddings=True)
    return np.asarray(vec[0], dtype=np.float32)


def cosine_sim(a: np.ndarray, b: np.ndarray) -> float:
    denom = float(np.linalg.norm(a) * np.linalg.norm(b))
    if denom == 0.0:
        return 0.0
    return float(np.dot(a, b) / denom)


def load_index() -> list[str]:
    raw = kv_command("GET", INDEX_KEY)
    if not raw:
        return []
    try:
        data = json.loads(raw)
        return data if isinstance(data, list) else []
    except json.JSONDecodeError:
        return []


def save_index(ids: list[str]) -> None:
    kv_command("SET", INDEX_KEY, json.dumps(ids))


@app.get("/health")
def health() -> dict:
    return {
        "status": "ok",
        "device": DEVICE,
        "model": MODEL_NAME,
        "chronokv": f"{CHRONOKV_HOST}:{CHRONOKV_PORT}",
    }


@app.post("/cache/lookup")
def lookup_cache(req: QueryRequest) -> dict:
    threshold = req.threshold if req.threshold is not None else SIMILARITY_THRESHOLD
    q_vec = embed_text(req.query)
    ids = load_index()

    best_score = -1.0
    best_entry: Optional[dict] = None

    for item_id in ids:
        raw_entry = kv_command("GET", f"semantic:item:{item_id}")
        if not raw_entry:
            continue
        try:
            entry = json.loads(raw_entry)
            vec = np.asarray(entry["embedding"], dtype=np.float32)
        except (KeyError, ValueError, TypeError, json.JSONDecodeError):
            continue

        score = cosine_sim(q_vec, vec)
        if score > best_score:
            best_score = score
            best_entry = entry

    if best_entry is not None and best_score >= threshold:
        return {
            "hit": True,
            "score": round(best_score, 4),
            "matched_query": best_entry.get("query"),
            "response": best_entry.get("response"),
        }

    return {
        "hit": False,
        "score": round(best_score, 4) if best_score >= 0 else 0.0,
        "threshold": threshold,
    }


@app.post("/cache/store")
def store_cache(req: QueryRequest) -> dict:
    if not req.response:
        return {"stored": False, "error": "response is required to store a cache entry"}

    vec = embed_text(req.query)
    ids = load_index()
    item_id = str(len(ids) + 1)

    record = {
        "id": item_id,
        "query": req.query,
        "response": req.response,
        "embedding": vec.tolist(),
    }

    kv_command("SET", f"semantic:item:{item_id}", json.dumps(record))
    ids.append(item_id)
    save_index(ids)

    return {
        "stored": True,
        "id": item_id,
        "device": DEVICE,
    }
