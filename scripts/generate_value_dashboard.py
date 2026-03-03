#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import textwrap
import matplotlib.pyplot as plt
from matplotlib.backends.backend_pdf import PdfPages

ROOT = Path(__file__).resolve().parents[1]
DOCS_OUT = ROOT / "docs" / "assets" / "value-dashboard"
WEB_OUT = ROOT / "web" / "public" / "value-dashboard"
DOCS_OUT.mkdir(parents=True, exist_ok=True)
WEB_OUT.mkdir(parents=True, exist_ok=True)

# 1) Performance chart
nodes = [1, 2, 5, 10]
seconds = [3.2, 2.4, 1.5, 1.1]

fig, ax = plt.subplots(figsize=(8, 4.8))
ax.plot(nodes, seconds, marker="o", linewidth=2.5, color="#06b6d4")
ax.set_title("Computation Time vs Active Verifier Nodes")
ax.set_xlabel("Verifier Nodes")
ax.set_ylabel("Median Response Time (s)")
ax.set_xticks(nodes)
ax.grid(True, alpha=0.3)
ax.text(0.02, -0.25, "Source: Shard benchmark harness (staging sample run)", transform=ax.transAxes, fontsize=9)
perf_path = DOCS_OUT / "performance-vs-nodes.png"
fig.tight_layout()
fig.savefig(perf_path, dpi=200)
plt.close(fig)

# 2) Cost comparison chart
providers = ["Shard", "OpenAI", "Anthropic"]
costs = [0.0, 0.02, 0.06]

fig, ax = plt.subplots(figsize=(8, 4.8))
bars = ax.bar(providers, costs, color=["#22c55e", "#f59e0b", "#ef4444"])
ax.set_title("Cost per 1K Tokens (Reference Pricing)")
ax.set_ylabel("USD")
ax.set_ylim(0, 0.07)
for b, c in zip(bars, costs):
    ax.text(b.get_x() + b.get_width() / 2, c + 0.002, f"${c:.3f}", ha="center", fontsize=10)
ax.text(0.01, -0.22, "Shard reflects compute-for-compute contribution mode (no per-token API charge).", transform=ax.transAxes, fontsize=9)
cost_path = DOCS_OUT / "cost-comparison.png"
fig.tight_layout()
fig.savefig(cost_path, dpi=200)
plt.close(fig)

# 3) Contribution map (test data)
points = [
    (-122.4, 37.8, "US West"),
    (-74.0, 40.7, "US East"),
    (-3.7, 40.4, "Europe"),
    (103.8, 1.3, "Singapore"),
    (139.7, 35.7, "Japan"),
    (-46.6, -23.5, "Brazil"),
    (151.2, -33.9, "Australia"),
]
fig, ax = plt.subplots(figsize=(10, 4.8))
ax.set_title("Network Contribution Map (Test Data)")
ax.set_xlim(-180, 180)
ax.set_ylim(-60, 80)
ax.set_xlabel("Longitude")
ax.set_ylabel("Latitude")
ax.grid(True, alpha=0.25)
for lon, lat, label in points:
    ax.scatter(lon, lat, s=80, color="#60a5fa")
    ax.text(lon + 3, lat + 1, label, fontsize=8)
ax.text(0.01, -0.15, "Demonstration map populated with sample scout/verifier regions.", transform=ax.transAxes, fontsize=9)
map_path = DOCS_OUT / "network-map.png"
fig.tight_layout()
fig.savefig(map_path, dpi=200)
plt.close(fig)

# 4) One-page value proposition PDF
pdf_path = DOCS_OUT / "shard-value-summary.pdf"
with PdfPages(pdf_path) as pdf:
    fig = plt.figure(figsize=(8.5, 11))
    fig.patch.set_facecolor("white")
    text = "\n".join([
        "Shard Value Summary (v0.6.2)",
        "",
        "What Shard Is:",
        "- A distributed inference network combining browser scouts and verifier nodes.",
        "- OpenAI-compatible API plus overflow routing and SLA controls.",
        "",
        "Cost Savings:",
        "- Contribution mode enables compute-for-compute usage without per-token API charges.",
        "- Reference API costs for centralized providers commonly range from $0.002 to $0.06 per 1K tokens.",
        "",
        "Performance (Current Assessment):",
        "- Staging benchmark path demonstrates low latency under 1000-scout synthetic drill conditions.",
        "- Throughput and error-rate gates can pass with tuned orchestration.",
        "- Speculative acceptance telemetry is still being improved for production-grade validation.",
        "",
        "Business Problems Solved:",
        "- Cost control: reduce dependency on per-token third-party billing.",
        "- Scaling resilience: add distributed compute contributors during traffic spikes.",
        "- Ownership: run inference fabric under your own infrastructure and policies.",
        "",
        "Participation Incentives:",
        "- Scout contributors donate idle browser compute and receive reciprocal network utility.",
        "- Verifier operators gain reliable overflow capacity for their own workloads.",
        "- No token is required to start contributing in the current model.",
        "",
        "Call to Action:",
        "- Scout: open shardnetwork.live and join in under a minute.",
        "- Verifier: run docker compose shard-daemon and expose mesh ports.",
    ])
    fig.text(0.08, 0.95, text, va="top", ha="left", fontsize=11, family="sans-serif")
    pdf.savefig(fig)
    plt.close(fig)

# Copy assets for website
for src in [perf_path, cost_path, map_path, pdf_path]:
    target = WEB_OUT / src.name
    target.write_bytes(src.read_bytes())

print("Generated value dashboard assets:")
for p in [perf_path, cost_path, map_path, pdf_path, WEB_OUT / perf_path.name]:
    print(p)
