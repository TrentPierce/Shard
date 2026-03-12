#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path
import matplotlib.pyplot as plt
from matplotlib.backends.backend_pdf import PdfPages

ROOT = Path(__file__).resolve().parents[1]
DOCS_OUT = ROOT / "docs" / "assets" / "value-dashboard"
WEB_OUT = ROOT / "web" / "public" / "value-dashboard"
DOCS_OUT.mkdir(parents=True, exist_ok=True)
WEB_OUT.mkdir(parents=True, exist_ok=True)

TARGET_NODE_COUNTS = [1, 2, 5, 10]


def load_json(path: Path) -> dict | None:
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def latest_node_latency_seconds() -> tuple[dict[int, float], list[str]]:
    """Return validated p95 latency seconds keyed by node count."""
    summaries = sorted(
        (ROOT / "benchmarks" / "results").glob("**/summary.json"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    chosen: dict[int, float] = {}
    sources: list[str] = []

    for summary in summaries:
        payload = load_json(summary)
        if not payload:
            continue
        scenarios = payload.get("scenarios")
        if not isinstance(scenarios, list):
            continue
        for scenario in scenarios:
            if not isinstance(scenario, dict):
                continue
            node_count = scenario.get("node_count")
            p95_ms = scenario.get("latency_p95_ms_mean")
            name = str(scenario.get("scenario", "")).lower()
            if not isinstance(node_count, int) or node_count not in TARGET_NODE_COUNTS:
                continue
            if not isinstance(p95_ms, (int, float)):
                continue
            # Prefer local-daemon evidence if present.
            if node_count not in chosen or "local" in name:
                chosen[node_count] = float(p95_ms) / 1000.0
                if len(sources) < 4:
                    sources.append(str(summary.relative_to(ROOT)))
        if all(node in chosen for node in TARGET_NODE_COUNTS):
            break

    return chosen, sources


def read_phase3_metric(path: Path, label: str) -> str:
    data = load_json(path)
    if not data:
        return f"- {label}: not available."
    acc = data.get("acceptance_rate_pct")
    err = data.get("error_rate_pct")
    p95 = data.get("p95_latency_ms")
    requests = data.get("total_requests")
    return (
        f"- {label}: acceptance {acc:.4g}% | error {err:.4g}% | "
        f"p95 {p95:.4g} ms | requests {requests}."
    )


def read_latest_sla_lines() -> list[str]:
    reports = sorted((ROOT / "reports").glob("sla-report-*.md"), key=lambda p: p.stat().st_mtime, reverse=True)
    if not reports:
        return ["- SLA report: not available."]
    text = reports[0].read_text(encoding="utf-8")
    rows = []
    for line in text.splitlines():
        if not line.startswith("|") or "Metric" in line or "---" in line:
            continue
        cols = [col.strip() for col in line.strip("|").split("|")]
        if len(cols) >= 4:
            rows.append(f"- {cols[0]}: {cols[1]} vs {cols[2]} -> {cols[3]}")
    if not rows:
        return [f"- SLA report parsed with no table rows: {reports[0].relative_to(ROOT)}"]
    return rows

validated_seconds, validated_sources = latest_node_latency_seconds()
nodes = TARGET_NODE_COUNTS
seconds = [validated_seconds.get(node) for node in nodes]

# 1) Performance chart (validated-only; missing counts are marked pending)
fig, ax = plt.subplots(figsize=(8, 4.8))
bar_heights = [value if value is not None else 0 for value in seconds]
bars = ax.bar(nodes, bar_heights, color=["#0891b2" if value is not None else "#475569" for value in seconds])
for idx, (bar, value) in enumerate(zip(bars, seconds)):
    if value is None:
        bar.set_hatch("//")
        ax.text(bar.get_x() + bar.get_width() / 2, 0.1, "pending\nvalidation", ha="center", va="bottom", fontsize=8)
    else:
        ax.text(bar.get_x() + bar.get_width() / 2, value + 0.15, f"{value:.2f}s", ha="center", va="bottom", fontsize=9)
ax.set_title("Validated p95 Response Time by Node Count")
ax.set_xlabel("Verifier Nodes")
ax.set_ylabel("p95 Response Time (s)")
ax.set_xticks(nodes)
ax.set_ylim(0, max([value for value in seconds if value is not None] + [1]) * 1.35)
ax.grid(True, axis="y", alpha=0.3)
source_label = (
    f"Sources: {', '.join(validated_sources[:2])}"
    if validated_sources
    else "Sources: benchmark summary files not found"
)
ax.text(
    0.02,
    -0.27,
    f"{source_label}. Missing node-count points are intentionally unfilled.",
    transform=ax.transAxes,
    fontsize=8.5,
)
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
phase3_local = read_phase3_metric(ROOT / "benchmarks" / "results" / "phase3-local-1000.json", "Phase 3 local 1000-scout")
phase3_ec2_quick = read_phase3_metric(ROOT / "benchmarks" / "results" / "phase3-ec2-quick.json", "Phase 3 EC2 1000-scout quick")
phase3_ec2 = read_phase3_metric(ROOT / "benchmarks" / "results" / "phase3-ec2.json", "Phase 3 EC2 100-scout baseline")
sla_lines = read_latest_sla_lines()

with PdfPages(pdf_path) as pdf:
    fig = plt.figure(figsize=(8.5, 11))
    fig.patch.set_facecolor("white")
    text = "\n".join([
        "Shard Value Summary (v0.6.6)",
        "",
        "What Shard Is:",
        "- A distributed inference network combining browser scouts and verifier nodes.",
        "- OpenAI-compatible API plus overflow routing and SLA controls.",
        "",
        "Cost Savings:",
        "- Contribution mode enables compute-for-compute usage without per-token API charges.",
        "- Reference API costs for centralized providers commonly range from $0.002 to $0.06 per 1K tokens.",
        "",
        "Performance (Validated Current Assessment):",
        "- Only validated node-count benchmark points are plotted; missing points are marked pending.",
        phase3_local,
        phase3_ec2_quick,
        phase3_ec2,
        "",
        "Phase 4 SLA Snapshot (latest report):",
        *sla_lines,
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
    fig.text(0.08, 0.95, text, va="top", ha="left", fontsize=10.5, family="sans-serif")
    pdf.savefig(fig)
    plt.close(fig)

# Copy assets for website
for src in [perf_path, cost_path, map_path, pdf_path]:
    target = WEB_OUT / src.name
    target.write_bytes(src.read_bytes())

print("Generated value dashboard assets:")
for p in [perf_path, cost_path, map_path, pdf_path, WEB_OUT / perf_path.name]:
    print(p)

