#!/usr/bin/env python3
"""
Automated local pipeline harness for mesh-scale benchmarking.

This script brings up 1/3/5 node mesh topologies using
deploy/testnet/docker-compose.pipeline.yml and runs the real benchmark tool.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

import requests


SCENARIO_MATRIX = {
    1: {
        "name": "single-node",
        "services": ["node-a"],
        "health_urls": ["http://127.0.0.1:19091/health"],
    },
    3: {
        "name": "three-node-mesh",
        "services": ["node-a", "node-b", "node-c"],
        "health_urls": [
            "http://127.0.0.1:19091/health",
            "http://127.0.0.1:29091/health",
            "http://127.0.0.1:39091/health",
        ],
    },
    5: {
        "name": "five-node-mesh",
        "services": ["node-a", "node-b", "node-c", "node-d", "node-e"],
        "health_urls": [
            "http://127.0.0.1:19091/health",
            "http://127.0.0.1:29091/health",
            "http://127.0.0.1:39091/health",
            "http://127.0.0.1:49091/health",
            "http://127.0.0.1:59091/health",
        ],
        "profile": "five-node",
    },
}


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(cmd, cwd=cwd, check=True)


def wait_health(urls: list[str], timeout_s: float) -> None:
    deadline = time.time() + timeout_s
    pending = set(urls)
    while pending and time.time() < deadline:
        still_pending = set()
        for url in pending:
            try:
                response = requests.get(url, timeout=3)
                if response.ok:
                    payload = response.json()
                    if payload.get("status") in {"ok", "degraded"}:
                        continue
            except Exception:  # noqa: BLE001
                pass
            still_pending.add(url)
        pending = still_pending
        if pending:
            time.sleep(2)
    if pending:
        raise RuntimeError(f"Timed out waiting for health: {sorted(pending)}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Run local pipeline scale benchmark")
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root containing deploy/testnet/docker-compose.pipeline.yml",
    )
    parser.add_argument(
        "--node-sizes",
        default="1,3,5",
        help="Comma-separated node sizes (subset of 1,3,5)",
    )
    parser.add_argument("--runs-per-scenario", type=int, default=5)
    parser.add_argument("--requests-per-run", type=int, default=40)
    parser.add_argument("--warmup-requests", type=int, default=5)
    parser.add_argument("--concurrency", type=int, default=8)
    parser.add_argument("--request-timeout-s", type=float, default=45.0)
    parser.add_argument("--health-timeout-s", type=float, default=120.0)
    parser.add_argument("--out-dir", default="benchmarks/results")
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    compose = repo_root / "deploy" / "testnet" / "docker-compose.pipeline.yml"
    benchmark_script = repo_root / "benchmarks" / "mesh_scale_benchmark.py"

    sizes = []
    for raw in args.node_sizes.split(","):
        size = int(raw.strip())
        if size not in SCENARIO_MATRIX:
            raise ValueError(f"Unsupported node size: {size}")
        sizes.append(size)
    if not sizes:
        raise ValueError("No node sizes selected")

    scenarios: list[dict[str, Any]] = []
    try:
        for size in sizes:
            matrix = SCENARIO_MATRIX[size]
            down_cmd = [
                "docker",
                "compose",
                "-f",
                str(compose),
                "down",
                "--remove-orphans",
                "-v",
            ]
            run(down_cmd, repo_root)

            up_cmd = ["docker", "compose", "-f", str(compose)]
            profile = matrix.get("profile")
            if profile:
                up_cmd.extend(["--profile", str(profile)])
            up_cmd.extend(["up", "-d"])
            up_cmd.extend(matrix["services"])
            run(up_cmd, repo_root)
            wait_health(matrix["health_urls"], args.health_timeout_s)

            scenarios.append(
                {
                    "name": matrix["name"],
                    "node_count": size,
                    "base_url": "http://127.0.0.1:19091",
                }
            )

        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False, encoding="utf-8"
        ) as temp:
            json.dump({"scenarios": scenarios}, temp, indent=2)
            temp_scenarios = temp.name

        benchmark_cmd = [
            "python",
            str(benchmark_script),
            "--scenarios-json",
            temp_scenarios,
            "--out-dir",
            args.out_dir,
            "--runs-per-scenario",
            str(args.runs_per_scenario),
            "--requests-per-run",
            str(args.requests_per_run),
            "--warmup-requests",
            str(args.warmup_requests),
            "--concurrency",
            str(args.concurrency),
            "--request-timeout-s",
            str(args.request_timeout_s),
            "--seed",
            str(args.seed),
        ]
        run(benchmark_cmd, repo_root)
    finally:
        run(
            [
                "docker",
                "compose",
                "-f",
                str(compose),
                "down",
                "--remove-orphans",
                "-v",
            ],
            repo_root,
        )


if __name__ == "__main__":
    main()
