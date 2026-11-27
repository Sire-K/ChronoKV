from pathlib import Path
import matplotlib.pyplot as plt
import numpy as np

# Measured results from benchmark_compare.py (in milliseconds)
resp_labels = ["ChronoKV RESP\n(:6379)", "Redis RESP\n(:6380)"]
resp_p50 = [0.067, 0.073]
resp_p95 = [0.105, 0.087]
resp_p99 = [0.320, 0.102]

sem_labels = ["ChronoKV Semantic Cache\n(end-to-end + GPU)", "pgvector Top-1 Cosine\n(500 rows, precomputed)"]
sem_p50 = [8.137, 0.209]
sem_p95 = [14.094, 0.446]
sem_p99 = [15.355, 1.748]

fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(11, 4.5), dpi=160)
fig.patch.set_facecolor("#0f172a")

for ax in (ax1, ax2):
    ax.set_facecolor("#1e293b")
    ax.tick_params(colors="#e2e8f0")
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.spines["left"].set_color("#475569")
    ax.spines["bottom"].set_color("#475569")
    ax.grid(axis="y", linestyle="--", alpha=0.25, color="#94a3b8")

# Panel 1: Raw RESP SET/GET tail latency
x1 = np.arange(len(resp_labels))
w = 0.24
ax1.bar(x1 - w, resp_p50, width=w, label="p50", color="#38bdf8")
ax1.bar(x1, resp_p95, width=w, label="p95", color="#818cf8")
ax1.bar(x1 + w, resp_p99, width=w, label="p99", color="#f97316")
ax1.set_title("RESP SET/GET Latency (ms)", color="#f8fafc", fontsize=11, fontweight="bold")
ax1.set_xticks(x1)
ax1.set_xticklabels(resp_labels, color="#e2e8f0", fontsize=9)
ax1.set_ylabel("Latency (ms)", color="#e2e8f0")
ax1.legend(facecolor="#0f172a", edgecolor="#334155", labelcolor="#e2e8f0")

# Panel 2: Semantic Cache vs pgvector
x2 = np.arange(len(sem_labels))
ax2.bar(x2 - w, sem_p50, width=w, label="p50", color="#38bdf8")
ax2.bar(x2, sem_p95, width=w, label="p95", color="#818cf8")
ax2.bar(x2 + w, sem_p99, width=w, label="p99", color="#f97316")
ax2.set_title("Semantic Cache vs. pgvector Latency (ms)", color="#f8fafc", fontsize=11, fontweight="bold")
ax2.set_xticks(x2)
ax2.set_xticklabels(sem_labels, color="#e2e8f0", fontsize=9)
ax2.set_ylabel("Latency (ms)", color="#e2e8f0")
ax2.legend(facecolor="#0f172a", edgecolor="#334155", labelcolor="#e2e8f0")

plt.tight_layout()
out_path = Path(__file__).resolve().parent / "p99_latency.png"
plt.savefig(out_path)
print(f"Saved chart to {out_path}")
