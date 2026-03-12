#!/usr/bin/env python3
from __future__ import annotations

import argparse
import concurrent.futures
import json
import statistics
import subprocess
import time
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import requests


DEFAULT_PROMPT = (
    "Explain in three short paragraphs how Shard routes simple prompts locally, "
    "offloads heavier work to verifier nodes, and uses mesh forwarding when a healthier "
    "peer can serve the request faster."
)


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    idx = min(len(ordered) - 1, round((len(ordered) - 1) * p))
    return float(ordered[idx])


def load_fly_machines(app: str) -> list[dict[str, Any]]:
    payload = subprocess.check_output(
        ["flyctl", "machine", "list", "-a", app, "--json"],
        text=True,
    )
    machines = json.loads(payload)
    return [machine for machine in machines if machine.get("state") == "started"]


def health_for_machine(base_url: str, machine_id: str, timeout_s: float) -> dict[str, Any]:
    response = requests.get(
        f"{base_url.rstrip('/')}/health",
        headers={"fly-force-instance-id": machine_id},
        timeout=timeout_s,
    )
    response.raise_for_status()
    return response.json()


def run_one_request(
    *,
    base_url: str,
    machine_id: str,
    prompt: str,
    max_tokens: int,
    mode: str,
    timeout_s: float,
) -> dict[str, Any]:
    headers = {
        "Content-Type": "application/json",
        "X-Shard-Inference-Mode": "standard",
        "fly-force-instance-id": machine_id,
    }
    if mode == "pinned_local":
        headers["x-shard-mesh-forward"] = "false"

    body = {
        "model": "default",
        "stream": False,
        "max_tokens": max_tokens,
        "temperature": 0.0,
        "top_p": 1.0,
        "messages": [{"role": "user", "content": prompt}],
    }

    started = time.perf_counter()
    try:
        response = requests.post(
            f"{base_url.rstrip('/')}/v1/chat/completions",
            headers=headers,
            json=body,
            timeout=timeout_s,
        )
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        payload: Any
        try:
            payload = response.json()
        except ValueError:
            payload = response.text
        completion_tokens = (
            int(payload.get("usage", {}).get("completion_tokens", 0))
            if isinstance(payload, dict)
            else 0
        )
        error_detail = None
        if not response.ok:
            if isinstance(payload, dict):
                error_detail = payload
            elif payload:
                error_detail = {"body": payload[:800]}
        return {
            "success": response.ok,
            "status_code": response.status_code,
            "latency_ms": round(elapsed_ms, 3),
            "completion_tokens": completion_tokens,
            "served_by": response.headers.get("x-shard-served-by"),
            "mesh_forwarded": response.headers.get("x-shard-mesh-forwarded"),
            "mesh_decision": response.headers.get("x-shard-mesh-decision"),
            "mesh_detail": response.headers.get("x-shard-mesh-detail"),
            "mesh_forward_target": response.headers.get("x-shard-mesh-forward-target"),
            "mesh_target_tier": response.headers.get("x-shard-mesh-target-tier"),
            "mesh_forwarded_by": response.headers.get("x-shard-mesh-forwarded-by"),
            "mesh_candidates": response.headers.get("x-shard-mesh-candidates"),
            "mesh_eligible": response.headers.get("x-shard-mesh-eligible"),
            "mesh_probed": response.headers.get("x-shard-mesh-probed"),
            "mesh_scored": response.headers.get("x-shard-mesh-scored"),
            "mesh_filtered": response.headers.get("x-shard-mesh-filtered"),
            "response_model": payload.get("model") if isinstance(payload, dict) else None,
            "error": error_detail,
        }
    except Exception as exc:  # noqa: BLE001
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        return {
            "success": False,
            "status_code": 0,
            "latency_ms": round(elapsed_ms, 3),
            "completion_tokens": 0,
            "served_by": None,
            "mesh_forwarded": None,
            "mesh_decision": None,
            "mesh_detail": None,
            "mesh_forward_target": None,
            "mesh_target_tier": None,
            "mesh_forwarded_by": None,
            "mesh_candidates": None,
            "mesh_eligible": None,
            "mesh_probed": None,
            "mesh_scored": None,
            "mesh_filtered": None,
            "response_model": None,
            "error": str(exc),
        }


def summarize_events(events: list[dict[str, Any]]) -> dict[str, Any]:
    successes = [event for event in events if event["success"]]
    latencies = [float(event["latency_ms"]) for event in successes]
    completion_tokens = sum(int(event["completion_tokens"]) for event in successes)
    served_by_counts = Counter(
        event["served_by"] for event in successes if event.get("served_by")
    )
    target_counts = Counter(
        event["mesh_forward_target"]
        for event in successes
        if event.get("mesh_forward_target")
    )
    model_counts = Counter(
        event["response_model"] for event in successes if event.get("response_model")
    )
    decision_counts = Counter(
        event["mesh_decision"] for event in events if event.get("mesh_decision")
    )
    status_counts = Counter(str(event["status_code"]) for event in events)
    forwarded_hits = sum(
        1 for event in successes if str(event.get("mesh_forwarded", "")).lower() == "true"
    )
    return {
        "requests": len(events),
        "successes": len(successes),
        "failures": len(events) - len(successes),
        "success_rate": round(len(successes) / len(events), 4) if events else 0.0,
        "avg_ms": round(statistics.fmean(latencies), 3) if latencies else 0.0,
        "p50_ms": round(percentile(latencies, 0.50), 3) if latencies else 0.0,
        "p95_ms": round(percentile(latencies, 0.95), 3) if latencies else 0.0,
        "completion_tokens_total": completion_tokens,
        "forwarded_hits": forwarded_hits,
        "forwarded_rate": round(forwarded_hits / len(successes), 4) if successes else 0.0,
        "served_by_counts": dict(served_by_counts),
        "mesh_decision_counts": dict(decision_counts),
        "mesh_target_counts": dict(target_counts),
        "response_model_counts": dict(model_counts),
        "status_counts": dict(status_counts),
    }


def run_mode(
    *,
    base_url: str,
    machine_id: str,
    prompt: str,
    max_tokens: int,
    mode: str,
    requests_per_run: int,
    concurrency: int,
    timeout_s: float,
) -> dict[str, Any]:
    events: list[dict[str, Any]] = []
    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [
            executor.submit(
                run_one_request,
                base_url=base_url,
                machine_id=machine_id,
                prompt=prompt,
                max_tokens=max_tokens,
                mode=mode,
                timeout_s=timeout_s,
            )
            for _ in range(requests_per_run)
        ]
        for future in concurrent.futures.as_completed(futures):
            events.append(future.result())
    elapsed_s = max(time.perf_counter() - started, 0.001)
    summary = summarize_events(events)
    summary["throughput_rps"] = round(summary["successes"] / elapsed_s, 4)
    return {
        "mode": mode,
        "elapsed_s": round(elapsed_s, 3),
        "summary": summary,
        "events": events,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Benchmark the Fly multi-node product path")
    parser.add_argument("--app", default="shard-fly-bench-0308c")
    parser.add_argument("--base-url", default="https://shard-fly-bench-0308c.fly.dev")
    parser.add_argument("--machine-ids", default="")
    parser.add_argument("--requests-per-run", type=int, default=6)
    parser.add_argument("--concurrency", type=int, default=2)
    parser.add_argument("--timeout-s", type=float, default=90.0)
    parser.add_argument("--max-tokens", type=int, default=160)
    parser.add_argument("--prompt", default=DEFAULT_PROMPT)
    parser.add_argument(
        "--output",
        default="runtime/fly_product_mesh_benchmark.json",
    )
    args = parser.parse_args()

    requested_machine_ids = {
        item.strip() for item in args.machine_ids.split(",") if item.strip()
    }
    machines = load_fly_machines(args.app)
    if requested_machine_ids:
        machines = [m for m in machines if m.get("id") in requested_machine_ids]
    if not machines:
        raise SystemExit("No Fly machines available for benchmark")

    machine_summaries: list[dict[str, Any]] = []
    aggregate_by_mode: dict[str, list[dict[str, Any]]] = defaultdict(list)

    for machine in machines:
        machine_id = str(machine["id"])
        region = str(machine.get("region", "unknown"))
        health = health_for_machine(args.base_url, machine_id, args.timeout_s)
        machine_result = {
            "machine_id": machine_id,
            "region": region,
            "health": health,
            "modes": [],
        }
        print(f"[{region}/{machine_id}] rust={health.get('rust_version')} model={health.get('model_id')}")
        for mode in ("pinned_local", "pinned_mesh"):
            result = run_mode(
                base_url=args.base_url,
                machine_id=machine_id,
                prompt=args.prompt,
                max_tokens=args.max_tokens,
                mode=mode,
                requests_per_run=args.requests_per_run,
                concurrency=args.concurrency,
                timeout_s=args.timeout_s,
            )
            machine_result["modes"].append(result)
            aggregate_by_mode[mode].extend(result["events"])
            summary = result["summary"]
            print(
                f"  [{mode}] success={summary['successes']}/{summary['requests']} "
                f"avg={summary['avg_ms']}ms p95={summary['p95_ms']}ms "
                f"forwarded_rate={summary['forwarded_rate']}"
            )
        machine_summaries.append(machine_result)

    aggregate_summary = {
        mode: summarize_events(events) for mode, events in aggregate_by_mode.items()
    }

    report = {
        "created_at": now_iso(),
        "app": args.app,
        "base_url": args.base_url,
        "requests_per_run": args.requests_per_run,
        "concurrency": args.concurrency,
        "timeout_s": args.timeout_s,
        "max_tokens": args.max_tokens,
        "prompt": args.prompt,
        "machines": machine_summaries,
        "aggregate_summary": aggregate_summary,
    }

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"Saved benchmark report to {output_path}")


if __name__ == "__main__":
    main()
