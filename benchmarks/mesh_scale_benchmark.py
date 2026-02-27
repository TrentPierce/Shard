#!/usr/bin/env python3
"""
Real mesh-scale benchmark runner for Shard.

This tool intentionally avoids synthetic estimates and random acceptance simulation.
All reported values are measured from live API responses and /metrics/summary counters.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import hashlib
import json
import math
import random
import re
import statistics
import subprocess
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import requests


DEFAULT_PROMPT = (
    "You are benchmarking distributed inference. Respond with three short bullets on "
    "why speculative decoding can reduce verifier work."
)


@dataclass
class Scenario:
    name: str
    node_count: int
    base_url: str


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def safe_get_json(url: str, timeout_s: float = 10.0) -> dict[str, Any]:
    response = requests.get(url, timeout=timeout_s)
    response.raise_for_status()
    return response.json()


def safe_get_json_retry(
    url: str,
    timeout_s: float = 10.0,
    retries: int = 4,
    backoff_s: float = 0.8,
) -> dict[str, Any]:
    last_error: Exception | None = None
    for attempt in range(retries + 1):
        try:
            return safe_get_json(url, timeout_s=timeout_s)
        except Exception as exc:  # noqa: BLE001
            last_error = exc
            if attempt < retries:
                time.sleep(backoff_s * (attempt + 1))
    raise RuntimeError(f"Failed to fetch JSON from {url}") from last_error


def safe_get_json_optional(
    url: str,
    timeout_s: float = 10.0,
    retries: int = 2,
) -> dict[str, Any]:
    try:
        return safe_get_json_retry(url, timeout_s=timeout_s, retries=retries)
    except Exception as exc:  # noqa: BLE001
        return {"status": "unavailable", "error": str(exc), "url": url}


def get_metrics_summary(base_url: str) -> dict[str, Any]:
    candidates = [
        f"{base_url}/metrics/summary",
        f"{base_url}/v1/metrics/summary",
    ]
    last_error: Exception | None = None
    for url in candidates:
        try:
            return safe_get_json_retry(url, retries=4)
        except Exception as exc:  # noqa: BLE001
            last_error = exc
    raise RuntimeError(f"Unable to fetch metrics summary from {candidates}") from last_error


def parse_scenarios(path: Path) -> list[Scenario]:
    data = json.loads(path.read_text(encoding="utf-8"))
    scenarios: list[Scenario] = []
    for item in data.get("scenarios", []):
        scenarios.append(
            Scenario(
                name=str(item["name"]),
                node_count=int(item["node_count"]),
                base_url=str(item["base_url"]).rstrip("/"),
            )
        )
    if not scenarios:
        raise ValueError("No scenarios found in scenarios JSON")
    return scenarios


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    sorted_values = sorted(values)
    idx = int(round((len(sorted_values) - 1) * p))
    return sorted_values[idx]


def confidence_interval_95(values: list[float]) -> tuple[float, float]:
    if not values:
        return 0.0, 0.0
    mean = statistics.fmean(values)
    if len(values) < 2:
        return mean, mean
    sd = statistics.stdev(values)
    margin = 1.96 * (sd / math.sqrt(len(values)))
    return mean - margin, mean + margin


def count_output_tokens(payload: dict[str, Any]) -> int:
    choices = payload.get("choices")
    if isinstance(choices, list) and choices:
        first = choices[0]
        if isinstance(first, dict):
            msg = first.get("message")
            if isinstance(msg, dict):
                content = msg.get("content", "")
                if isinstance(content, str):
                    return len(content.split())
    text = json.dumps(payload, ensure_ascii=True)
    return len(text.split())


def classify_error(status_code: int, error: str) -> str:
    err = (error or "").lower()
    if not err and 200 <= status_code < 400:
        return "none"
    if status_code == 0 and "timed out" in err:
        return "connection_timeout"
    if status_code == 0 and ("failed to establish" in err or "connection" in err):
        return "connection_failure"
    if status_code == 0 and ("name or service not known" in err or "dns" in err):
        return "dns_failure"
    if status_code == 408:
        return "inference_timeout"
    if status_code == 429:
        return "rate_limited"
    if status_code == 502:
        return "bad_gateway"
    if status_code == 503:
        return "service_unavailable"
    if status_code == 504:
        return "gateway_timeout"
    if 500 <= status_code < 600:
        return "server_error"
    if 400 <= status_code < 500:
        return "client_error"
    if status_code >= 400:
        return "http_error"
    if err:
        return "network_error"
    return "unknown"


def parse_sse_data_line(line: str) -> dict[str, Any] | None:
    line = line.strip()
    if not line.startswith("data:"):
        return None
    payload = line[5:].strip()
    if not payload or payload == "[DONE]":
        return None
    try:
        return json.loads(payload)
    except json.JSONDecodeError:
        return None


def run_one_request_stream(
    base_url: str,
    inference_mode: str,
    prompt: str,
    timeout_s: float,
    max_tokens: int,
) -> dict[str, Any]:
    request_id = str(uuid.uuid4())
    body = {
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": prompt}],
        "stream": True,
        "max_tokens": max_tokens,
    }
    headers = {
        "Content-Type": "application/json",
        "X-Shard-Inference-Mode": inference_mode,
    }
    started = time.perf_counter()
    success = False
    status_code = 0
    output_tokens = 0
    error = ""
    ttft_ms: float | None = None
    inter_token_latency_ms: float | None = None
    token_ts: list[float] = []
    try:
        with requests.post(
            f"{base_url}/v1/chat/completions",
            json=body,
            headers=headers,
            timeout=timeout_s,
            stream=True,
        ) as response:
            status_code = response.status_code
            response.raise_for_status()
            for raw_line in response.iter_lines(decode_unicode=True):
                if not raw_line:
                    continue
                item = parse_sse_data_line(raw_line)
                if not isinstance(item, dict):
                    continue
                choices = item.get("choices")
                if not isinstance(choices, list) or not choices:
                    continue
                choice0 = choices[0] if isinstance(choices[0], dict) else {}
                delta = choice0.get("delta") if isinstance(choice0, dict) else None
                content = delta.get("content") if isinstance(delta, dict) else None
                if isinstance(content, str) and content:
                    now = time.perf_counter()
                    token_ts.append(now)
                    output_tokens += max(1, len(re.findall(r"\S+", content)))
            success = True
    except Exception as exc:  # noqa: BLE001
        error = str(exc)

    if token_ts:
        ttft_ms = (token_ts[0] - started) * 1000.0
        if len(token_ts) >= 2:
            gaps = [(token_ts[i] - token_ts[i - 1]) * 1000.0 for i in range(1, len(token_ts))]
            inter_token_latency_ms = statistics.fmean(gaps)

    elapsed_ms = (time.perf_counter() - started) * 1000.0
    return {
        "request_id": request_id,
        "latency_ms": elapsed_ms,
        "ttft_ms": ttft_ms,
        "inter_token_latency_ms": inter_token_latency_ms,
        "success": success,
        "status_code": status_code,
        "output_tokens": output_tokens,
        "error": error,
        "error_class": classify_error(status_code, error),
        "transport_mode": "stream_sse",
    }


def infer_transport_family(addr: str) -> str:
    lower = addr.lower()
    if "/webrtc-direct" in lower:
        return "webrtc_direct"
    if "/quic-v1" in lower:
        return "quic"
    if "/ws" in lower:
        return "websocket_tcp"
    if "/tcp/" in lower:
        return "tcp"
    if "/p2p-circuit" in lower:
        return "relay_circuit"
    return "other"


def collect_transport_observation(base_url: str) -> dict[str, Any]:
    peers = safe_get_json_optional(f"{base_url}/v1/system/peers", retries=1)
    topology = safe_get_json_optional(f"{base_url}/v1/system/topology", retries=1)

    protocol_counts: dict[str, int] = {}
    peer_count = 0
    for peer in peers.get("peers", []) if isinstance(peers, dict) else []:
        if not isinstance(peer, dict):
            continue
        addrs = peer.get("addrs")
        if not isinstance(addrs, list):
            continue
        peer_count += 1
        for addr in addrs:
            if not isinstance(addr, str):
                continue
            fam = infer_transport_family(addr)
            protocol_counts[fam] = protocol_counts.get(fam, 0) + 1

    local_protocol_addrs: dict[str, str] = {}
    if isinstance(topology, dict):
        for key in ("shard_webrtc_multiaddr", "shard_quic_multiaddr", "shard_ws_multiaddr"):
            value = topology.get(key)
            if isinstance(value, str) and value:
                local_protocol_addrs[key] = value

    return {
        "peer_count": peer_count,
        "observed_addr_counts": protocol_counts,
        "local_topology_addrs": local_protocol_addrs,
    }

def run_one_request(
    base_url: str,
    inference_mode: str,
    prompt: str,
    timeout_s: float,
    max_tokens: int,
) -> dict[str, Any]:
    request_id = str(uuid.uuid4())
    body = {
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": prompt}],
        "stream": False,
        "max_tokens": max_tokens,
    }
    headers = {
        "Content-Type": "application/json",
        "X-Shard-Inference-Mode": inference_mode,
    }
    started = time.perf_counter()
    success = False
    status_code = 0
    output_tokens = 0
    error = ""
    try:
        response = requests.post(
            f"{base_url}/v1/chat/completions",
            json=body,
            headers=headers,
            timeout=timeout_s,
        )
        status_code = response.status_code
        response.raise_for_status()
        payload = response.json()
        output_tokens = count_output_tokens(payload)
        success = True
    except Exception as exc:  # noqa: BLE001
        error = str(exc)
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    return {
        "request_id": request_id,
        "latency_ms": elapsed_ms,
        "ttft_ms": None,
        "inter_token_latency_ms": None,
        "success": success,
        "status_code": status_code,
        "output_tokens": output_tokens,
        "error": error,
        "error_class": classify_error(status_code, error),
        "transport_mode": "json",
    }


def benchmark_scenario(
    scenario: Scenario,
    run_index: int,
    requests_per_run: int,
    concurrency: int,
    warmup_requests: int,
    inference_mode: str,
    request_timeout_s: float,
    prompt: str,
    max_tokens: int,
    collect_latency_breakdown: bool,
) -> dict[str, Any]:
    metrics_before = get_metrics_summary(scenario.base_url)
    health_before = safe_get_json_optional(f"{scenario.base_url}/health")
    transport_before = collect_transport_observation(scenario.base_url)

    request_fn = run_one_request_stream if collect_latency_breakdown else run_one_request
    for _ in range(max(0, warmup_requests)):
        request_fn(
            scenario.base_url,
            inference_mode,
            prompt,
            timeout_s=request_timeout_s,
            max_tokens=max_tokens,
        )

    started = time.perf_counter()
    events: list[dict[str, Any]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [
            executor.submit(
                request_fn,
                scenario.base_url,
                inference_mode,
                prompt,
                request_timeout_s,
                max_tokens,
            )
            for _ in range(requests_per_run)
        ]
        for future in concurrent.futures.as_completed(futures):
            events.append(future.result())
    elapsed_s = max(0.001, time.perf_counter() - started)

    metrics_after = get_metrics_summary(scenario.base_url)
    health_after = safe_get_json_optional(f"{scenario.base_url}/health")
    transport_after = collect_transport_observation(scenario.base_url)

    successes = [e for e in events if e["success"]]
    failures = [e for e in events if not e["success"]]
    latencies = [float(e["latency_ms"]) for e in successes]
    ttft = [float(e["ttft_ms"]) for e in successes if e.get("ttft_ms") is not None]
    inter_token = [
        float(e["inter_token_latency_ms"])
        for e in successes
        if e.get("inter_token_latency_ms") is not None
    ]
    output_tokens = sum(int(e["output_tokens"]) for e in successes)

    error_distribution: dict[str, int] = {}
    for event in failures:
        klass = str(event.get("error_class") or "unknown")
        error_distribution[klass] = error_distribution.get(klass, 0) + 1

    accepted_before = int(metrics_before.get("speculative_accepted_tokens_total", 0))
    accepted_after = int(metrics_after.get("speculative_accepted_tokens_total", 0))
    rejected_before = int(metrics_before.get("speculative_rejected_tokens_total", 0))
    rejected_after = int(metrics_after.get("speculative_rejected_tokens_total", 0))
    drafts_before = int(metrics_before.get("speculative_draft_tokens_total", 0))
    drafts_after = int(metrics_after.get("speculative_draft_tokens_total", 0))

    delta_accepted = max(0, accepted_after - accepted_before)
    delta_rejected = max(0, rejected_after - rejected_before)
    delta_drafts = max(0, drafts_after - drafts_before)

    measured_acceptance = (
        float(delta_accepted) / float(delta_drafts) if delta_drafts > 0 else None
    )
    measured_reject = (
        float(delta_rejected) / float(delta_drafts) if delta_drafts > 0 else None
    )
    verification_fallback_total_delta = max(
        0,
        int(metrics_after.get("verification_fallback_total", 0))
        - int(metrics_before.get("verification_fallback_total", 0)),
    )
    scout_draft_submissions_total_delta = max(
        0,
        int(metrics_after.get("scout_draft_submissions_total", 0))
        - int(metrics_before.get("scout_draft_submissions_total", 0)),
    )
    scout_draft_reject_pow_total_delta = max(
        0,
        int(metrics_after.get("scout_draft_reject_pow_total", 0))
        - int(metrics_before.get("scout_draft_reject_pow_total", 0)),
    )
    scout_draft_reject_spotcheck_total_delta = max(
        0,
        int(metrics_after.get("scout_draft_reject_spotcheck_total", 0))
        - int(metrics_before.get("scout_draft_reject_spotcheck_total", 0)),
    )
    scout_draft_reject_empty_tokens_total_delta = max(
        0,
        int(metrics_after.get("scout_draft_reject_empty_tokens_total", 0))
        - int(metrics_before.get("scout_draft_reject_empty_tokens_total", 0)),
    )
    scout_client_submit_attempts_total_delta = max(
        0,
        int(metrics_after.get("scout_client_submit_attempts_total", 0))
        - int(metrics_before.get("scout_client_submit_attempts_total", 0)),
    )
    scout_client_submit_success_total_delta = max(
        0,
        int(metrics_after.get("scout_client_submit_success_total", 0))
        - int(metrics_before.get("scout_client_submit_success_total", 0)),
    )
    scout_client_submit_http_failures_total_delta = max(
        0,
        int(metrics_after.get("scout_client_submit_http_failures_total", 0))
        - int(metrics_before.get("scout_client_submit_http_failures_total", 0)),
    )
    scout_client_submit_network_failures_total_delta = max(
        0,
        int(metrics_after.get("scout_client_submit_network_failures_total", 0))
        - int(metrics_before.get("scout_client_submit_network_failures_total", 0)),
    )
    scout_client_submit_timeouts_total_delta = max(
        0,
        int(metrics_after.get("scout_client_submit_timeouts_total", 0))
        - int(metrics_before.get("scout_client_submit_timeouts_total", 0)),
    )
    scout_client_submit_pow_failures_total_delta = max(
        0,
        int(metrics_after.get("scout_client_submit_pow_failures_total", 0))
        - int(metrics_before.get("scout_client_submit_pow_failures_total", 0)),
    )
    scout_work_assignments_total_delta = max(
        0,
        int(metrics_after.get("scout_work_assignments_total", 0))
        - int(metrics_before.get("scout_work_assignments_total", 0)),
    )
    transport_tcp_success_total_delta = max(
        0,
        int(metrics_after.get("transport_tcp_success_total", 0))
        - int(metrics_before.get("transport_tcp_success_total", 0)),
    )
    transport_tcp_failure_total_delta = max(
        0,
        int(metrics_after.get("transport_tcp_failure_total", 0))
        - int(metrics_before.get("transport_tcp_failure_total", 0)),
    )
    transport_websocket_success_total_delta = max(
        0,
        int(metrics_after.get("transport_websocket_success_total", 0))
        - int(metrics_before.get("transport_websocket_success_total", 0)),
    )
    transport_websocket_failure_total_delta = max(
        0,
        int(metrics_after.get("transport_websocket_failure_total", 0))
        - int(metrics_before.get("transport_websocket_failure_total", 0)),
    )
    transport_quic_success_total_delta = max(
        0,
        int(metrics_after.get("transport_quic_success_total", 0))
        - int(metrics_before.get("transport_quic_success_total", 0)),
    )
    transport_quic_failure_total_delta = max(
        0,
        int(metrics_after.get("transport_quic_failure_total", 0))
        - int(metrics_before.get("transport_quic_failure_total", 0)),
    )
    transport_webrtc_success_total_delta = max(
        0,
        int(metrics_after.get("transport_webrtc_success_total", 0))
        - int(metrics_before.get("transport_webrtc_success_total", 0)),
    )
    transport_webrtc_failure_total_delta = max(
        0,
        int(metrics_after.get("transport_webrtc_failure_total", 0))
        - int(metrics_before.get("transport_webrtc_failure_total", 0)),
    )
    transport_relay_success_total_delta = max(
        0,
        int(metrics_after.get("transport_relay_success_total", 0))
        - int(metrics_before.get("transport_relay_success_total", 0)),
    )
    transport_relay_failure_total_delta = max(
        0,
        int(metrics_after.get("transport_relay_failure_total", 0))
        - int(metrics_before.get("transport_relay_failure_total", 0)),
    )

    speculative_zero_draft_fallback_reason: str | None = None
    if inference_mode.lower() in ("distributed", "speculative") and delta_drafts == 0:
        health_state = (
            str(health_after.get("status", "")).lower() if isinstance(health_after, dict) else ""
        )
        readiness_reason = (
            str(health_after.get("readiness_reason", ""))
            if isinstance(health_after, dict)
            else ""
        )
        active_scouts = (
            int(health_after.get("active_scouts", 0)) if isinstance(health_after, dict) else 0
        )
        draft_capable_scouts = (
            int(health_after.get("draft_capable_scouts", 0))
            if isinstance(health_after, dict)
            else 0
        )
        submit_failure_total = (
            scout_client_submit_http_failures_total_delta
            + scout_client_submit_network_failures_total_delta
            + scout_client_submit_timeouts_total_delta
            + scout_client_submit_pow_failures_total_delta
        )
        if health_state and health_state not in ("ok", "ready"):
            speculative_zero_draft_fallback_reason = (
                f"backend_not_ready:{health_state}:{readiness_reason or 'unknown_reason'}"
            )
        elif draft_capable_scouts == 0 and active_scouts == 0:
            speculative_zero_draft_fallback_reason = "no_draft_capable_scouts"
        elif scout_work_assignments_total_delta == 0:
            speculative_zero_draft_fallback_reason = "no_scout_work_assignments"
        elif (
            scout_client_submit_attempts_total_delta > 0
            and scout_client_submit_success_total_delta == 0
        ):
            if scout_client_submit_pow_failures_total_delta > 0:
                speculative_zero_draft_fallback_reason = "pow_rejections"
            elif submit_failure_total > 0:
                speculative_zero_draft_fallback_reason = "scout_submit_transport_failures"
            else:
                speculative_zero_draft_fallback_reason = "scout_submit_no_success"
        elif scout_draft_submissions_total_delta > 0 and delta_accepted == 0 and delta_rejected > 0:
            speculative_zero_draft_fallback_reason = "all_drafts_rejected_by_verifier"
        elif verification_fallback_total_delta > 0:
            speculative_zero_draft_fallback_reason = "verifier_fallback_path"

    return {
        "scenario": scenario.name,
        "node_count": scenario.node_count,
        "base_url": scenario.base_url,
        "run_index": run_index,
        "started_at_utc": now_iso(),
        "requests_total": requests_per_run,
        "concurrency": concurrency,
        "inference_mode": inference_mode,
        "elapsed_seconds": elapsed_s,
        "success_count": len(successes),
        "failure_count": len(failures),
        "success_rate": (len(successes) / requests_per_run) if requests_per_run else 0.0,
        "throughput_rps": len(successes) / elapsed_s,
        "output_tokens_total": output_tokens,
        "output_tokens_per_second": output_tokens / elapsed_s if output_tokens else 0.0,
        "latency_avg_ms": statistics.fmean(latencies) if latencies else 0.0,
        "latency_p50_ms": percentile(latencies, 0.50),
        "latency_p95_ms": percentile(latencies, 0.95),
        "latency_p99_ms": percentile(latencies, 0.99),
        "latency_breakdown": {
            "ttft_avg_ms": statistics.fmean(ttft) if ttft else None,
            "ttft_p95_ms": percentile(ttft, 0.95) if ttft else None,
            "inter_token_avg_ms": statistics.fmean(inter_token) if inter_token else None,
            "inter_token_p95_ms": percentile(inter_token, 0.95) if inter_token else None,
            "ttft_samples": len(ttft),
            "inter_token_samples": len(inter_token),
        },
        "error_distribution": error_distribution,
        "metrics_delta": {
            "speculative_draft_tokens_total": delta_drafts,
            "speculative_accepted_tokens_total": delta_accepted,
            "speculative_rejected_tokens_total": delta_rejected,
            "measured_acceptance_rate": measured_acceptance,
            "measured_reject_rate": measured_reject,
            "measured_speedup_ratio": metrics_after.get("speculative_speedup_ratio"),
            "verification_fallback_total_delta": verification_fallback_total_delta,
            "scout_draft_submissions_total_delta": scout_draft_submissions_total_delta,
            "scout_draft_reject_pow_total_delta": scout_draft_reject_pow_total_delta,
            "scout_draft_reject_spotcheck_total_delta": scout_draft_reject_spotcheck_total_delta,
            "scout_draft_reject_empty_tokens_total_delta": scout_draft_reject_empty_tokens_total_delta,
            "scout_client_submit_attempts_total_delta": scout_client_submit_attempts_total_delta,
            "scout_client_submit_success_total_delta": scout_client_submit_success_total_delta,
            "scout_client_submit_http_failures_total_delta": scout_client_submit_http_failures_total_delta,
            "scout_client_submit_network_failures_total_delta": scout_client_submit_network_failures_total_delta,
            "scout_client_submit_timeouts_total_delta": scout_client_submit_timeouts_total_delta,
            "scout_client_submit_pow_failures_total_delta": scout_client_submit_pow_failures_total_delta,
            "scout_work_assignments_total_delta": scout_work_assignments_total_delta,
            "speculative_zero_draft_fallback_reason": speculative_zero_draft_fallback_reason,
            "transport_tcp_success_total_delta": transport_tcp_success_total_delta,
            "transport_tcp_failure_total_delta": transport_tcp_failure_total_delta,
            "transport_websocket_success_total_delta": transport_websocket_success_total_delta,
            "transport_websocket_failure_total_delta": transport_websocket_failure_total_delta,
            "transport_quic_success_total_delta": transport_quic_success_total_delta,
            "transport_quic_failure_total_delta": transport_quic_failure_total_delta,
            "transport_webrtc_success_total_delta": transport_webrtc_success_total_delta,
            "transport_webrtc_failure_total_delta": transport_webrtc_failure_total_delta,
            "transport_relay_success_total_delta": transport_relay_success_total_delta,
            "transport_relay_failure_total_delta": transport_relay_failure_total_delta,
        },
        "transport_observation_before": transport_before,
        "transport_observation_after": transport_after,
        "health_before": health_before,
        "health_after": health_after,
        "metrics_before": metrics_before,
        "metrics_after": metrics_after,
        "request_events": events,
    }


def write_csv(path: Path, run_results: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            [
                "scenario",
                "node_count",
                "run_index",
                "request_id",
                "success",
                "status_code",
                "latency_ms",
                "ttft_ms",
                "inter_token_latency_ms",
                "output_tokens",
                "error_class",
                "error",
            ]
        )
        for run in run_results:
            for event in run["request_events"]:
                writer.writerow(
                    [
                        run["scenario"],
                        run["node_count"],
                        run["run_index"],
                        event["request_id"],
                        event["success"],
                        event["status_code"],
                        f"{event['latency_ms']:.3f}",
                        f"{event['ttft_ms']:.3f}" if event.get("ttft_ms") is not None else "",
                        (
                            f"{event['inter_token_latency_ms']:.3f}"
                            if event.get("inter_token_latency_ms") is not None
                            else ""
                        ),
                        event["output_tokens"],
                        event.get("error_class", ""),
                        event["error"],
                    ]
                )

def merge_error_distributions(runs: list[dict[str, Any]]) -> dict[str, int]:
    merged: dict[str, int] = {}
    for run in runs:
        dist = run.get("error_distribution", {})
        if not isinstance(dist, dict):
            continue
        for key, value in dist.items():
            merged[str(key)] = merged.get(str(key), 0) + int(value)
    return merged


def merge_transport_deltas(runs: list[dict[str, Any]]) -> dict[str, int]:
    keys = [
        "transport_tcp_success_total_delta",
        "transport_tcp_failure_total_delta",
        "transport_websocket_success_total_delta",
        "transport_websocket_failure_total_delta",
        "transport_quic_success_total_delta",
        "transport_quic_failure_total_delta",
        "transport_webrtc_success_total_delta",
        "transport_webrtc_failure_total_delta",
        "transport_relay_success_total_delta",
        "transport_relay_failure_total_delta",
    ]
    merged = {k: 0 for k in keys}
    for run in runs:
        delta = run.get("metrics_delta", {})
        if not isinstance(delta, dict):
            continue
        for key in keys:
            merged[key] += int(delta.get(key, 0))
    return merged


def summarize(run_results: list[dict[str, Any]]) -> dict[str, Any]:
    by_scenario: dict[str, list[dict[str, Any]]] = {}
    for run in run_results:
        by_scenario.setdefault(run["scenario"], []).append(run)

    scenarios_summary: list[dict[str, Any]] = []
    for scenario, runs in by_scenario.items():
        tps = [float(r["throughput_rps"]) for r in runs]
        tokps = [float(r["output_tokens_per_second"]) for r in runs]
        p95 = [float(r["latency_p95_ms"]) for r in runs]
        success_rates = [float(r["success_rate"]) for r in runs]
        ttft_samples = [
            float(r["latency_breakdown"]["ttft_avg_ms"])
            for r in runs
            if r["latency_breakdown"]["ttft_avg_ms"] is not None
        ]
        inter_token_samples = [
            float(r["latency_breakdown"]["inter_token_avg_ms"])
            for r in runs
            if r["latency_breakdown"]["inter_token_avg_ms"] is not None
        ]
        acceptance_rates = [
            r["metrics_delta"]["measured_acceptance_rate"]
            for r in runs
            if r["metrics_delta"]["measured_acceptance_rate"] is not None
        ]
        reject_rates = [
            r["metrics_delta"]["measured_reject_rate"]
            for r in runs
            if r["metrics_delta"]["measured_reject_rate"] is not None
        ]
        node_count = runs[0]["node_count"]
        ci_low, ci_high = confidence_interval_95(tps)
        tok_ci_low, tok_ci_high = confidence_interval_95(tokps)
        scenarios_summary.append(
            {
                "scenario": scenario,
                "node_count": node_count,
                "runs": len(runs),
                "throughput_rps_mean": statistics.fmean(tps),
                "throughput_rps_ci95_low": ci_low,
                "throughput_rps_ci95_high": ci_high,
                "output_tokens_per_second_mean": statistics.fmean(tokps),
                "output_tokens_per_second_ci95_low": tok_ci_low,
                "output_tokens_per_second_ci95_high": tok_ci_high,
                "latency_p95_ms_mean": statistics.fmean(p95),
                "ttft_avg_ms_mean": statistics.fmean(ttft_samples) if ttft_samples else None,
                "inter_token_avg_ms_mean": (
                    statistics.fmean(inter_token_samples) if inter_token_samples else None
                ),
                "success_rate_mean": statistics.fmean(success_rates),
                "measured_acceptance_rate_mean": (
                    statistics.fmean(acceptance_rates) if acceptance_rates else None
                ),
                "measured_reject_rate_mean": (
                    statistics.fmean(reject_rates) if reject_rates else None
                ),
                "error_distribution": merge_error_distributions(runs),
                "transport_deltas": merge_transport_deltas(runs),
            }
        )

    scenarios_summary.sort(key=lambda item: item["node_count"])
    return {"scenarios": scenarios_summary}


def speculative_gate_violations(
    run_results: list[dict[str, Any]],
    inference_mode: str,
) -> list[dict[str, Any]]:
    if inference_mode.lower() not in ("distributed", "speculative"):
        return []
    violations: list[dict[str, Any]] = []
    for run in run_results:
        delta = run.get("metrics_delta", {})
        drafts = int(delta.get("speculative_draft_tokens_total", 0))
        reason = delta.get("speculative_zero_draft_fallback_reason")
        if drafts == 0 and (not isinstance(reason, str) or not reason.strip()):
            violations.append(
                {
                    "scenario": str(run.get("scenario", "unknown")),
                    "run_index": int(run.get("run_index", -1)),
                    "base_url": str(run.get("base_url", "")),
                    "metrics_delta": delta,
                }
            )
    return violations


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def git_commit() -> str:
    try:
        return subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    except Exception:  # noqa: BLE001
        return "unknown"


def write_markdown_report(
    path: Path,
    summary: dict[str, Any],
    meta: dict[str, Any],
) -> None:
    lines = [
        "# Shard Mesh Scale Benchmark Report",
        "",
        f"- Timestamp (UTC): `{meta['timestamp_utc']}`",
        f"- Git commit: `{meta['git_commit']}`",
        f"- Inference mode: `{meta['inference_mode']}`",
        f"- Speculative CI gate required: `{meta['require_speculative_gate']}`",
        f"- Latency breakdown mode: `{'stream TTFT/inter-token' if meta['collect_latency_breakdown'] else 'request-only latency'}`",
        f"- Runs per scenario: `{meta['runs_per_scenario']}`",
        f"- Requests per run: `{meta['requests_per_run']}`",
        f"- Concurrency: `{meta['concurrency']}`",
        "",
        "## Methodology",
        "- Live HTTP requests to `/v1/chat/completions`.",
        "- Optional stream mode (`--collect-latency-breakdown`) measures TTFT and inter-token latency.",
        "- Metrics deltas captured from `/metrics/summary` before/after each run.",
        "- Transport observations sampled from `/v1/system/peers` and `/v1/system/topology`.",
        "- No synthetic acceptance/savings values are generated by this tool.",
        "- Scenario order is randomized each cycle to reduce warm-cache bias.",
        "",
        "## Scenario Summary",
        "",
        "| Scenario | Nodes | TPS Mean | TPS 95% CI | Token/s Mean | Token/s 95% CI | p95 Latency (ms) | TTFT Avg (ms) | Inter-token Avg (ms) | Success Rate | Acceptance | Reject |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for item in summary["scenarios"]:
        acc = (
            f"{item['measured_acceptance_rate_mean']:.4f}"
            if item["measured_acceptance_rate_mean"] is not None
            else "n/a"
        )
        rej = (
            f"{item['measured_reject_rate_mean']:.4f}"
            if item["measured_reject_rate_mean"] is not None
            else "n/a"
        )
        ttft = f"{item['ttft_avg_ms_mean']:.2f}" if item["ttft_avg_ms_mean"] is not None else "n/a"
        inter_token = (
            f"{item['inter_token_avg_ms_mean']:.2f}"
            if item["inter_token_avg_ms_mean"] is not None
            else "n/a"
        )
        lines.append(
            "| {scenario} | {node_count} | {tps:.3f} | [{lo:.3f}, {hi:.3f}] | "
            "{tokps:.3f} | [{tok_lo:.3f}, {tok_hi:.3f}] | {p95:.2f} | {ttft} | {inter_token} | {succ:.4f} | {acc} | {rej} |".format(
                scenario=item["scenario"],
                node_count=item["node_count"],
                tps=item["throughput_rps_mean"],
                lo=item["throughput_rps_ci95_low"],
                hi=item["throughput_rps_ci95_high"],
                tokps=item["output_tokens_per_second_mean"],
                tok_lo=item["output_tokens_per_second_ci95_low"],
                tok_hi=item["output_tokens_per_second_ci95_high"],
                p95=item["latency_p95_ms_mean"],
                ttft=ttft,
                inter_token=inter_token,
                succ=item["success_rate_mean"],
                acc=acc,
                rej=rej,
            )
        )
        if item.get("error_distribution"):
            lines.append(
                f"  - `{item['scenario']}` failure distribution: `{json.dumps(item['error_distribution'], sort_keys=True)}`"
            )
        if item.get("transport_deltas"):
            lines.append(
                f"  - `{item['scenario']}` transport deltas: `{json.dumps(item['transport_deltas'], sort_keys=True)}`"
            )

    lines.extend(
        [
            "",
            "## Investor Note",
            "- Use raw JSON/CSV + manifest hashes for due diligence.",
            "- Claims should reference confidence intervals, not single best runs.",
            "",
        ]
    )
    path.write_text("\n".join(lines), encoding="utf-8")

def main() -> None:
    parser = argparse.ArgumentParser(description="Run real mesh-scale benchmarks")
    parser.add_argument("--scenarios-json", required=True, help="Path to scenarios JSON")
    parser.add_argument("--out-dir", default="benchmarks/results", help="Output directory")
    parser.add_argument("--runs-per-scenario", type=int, default=5)
    parser.add_argument("--requests-per-run", type=int, default=40)
    parser.add_argument("--warmup-requests", type=int, default=5)
    parser.add_argument("--concurrency", type=int, default=8)
    parser.add_argument("--request-timeout-s", type=float, default=45.0)
    parser.add_argument("--inference-mode", default="distributed")
    parser.add_argument("--prompt", default=DEFAULT_PROMPT)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--max-tokens", type=int, default=48)
    parser.add_argument(
        "--collect-latency-breakdown",
        action="store_true",
        help="Use stream requests to measure TTFT and inter-token latency",
    )
    parser.add_argument(
        "--require-speculative-gate",
        action="store_true",
        help=(
            "Fail with non-zero exit when distributed/speculative runs report zero draft "
            "counter deltas without explicit fallback reasons"
        ),
    )
    args = parser.parse_args()

    random.seed(args.seed)
    scenarios = parse_scenarios(Path(args.scenarios_json))
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    run_stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = out_dir / f"mesh-scale-{run_stamp}"
    run_dir.mkdir(parents=True, exist_ok=True)

    run_results: list[dict[str, Any]] = []
    for run_index in range(1, args.runs_per_scenario + 1):
        order = scenarios[:]
        random.shuffle(order)
        for scenario in order:
            result = benchmark_scenario(
                scenario=scenario,
                run_index=run_index,
                requests_per_run=args.requests_per_run,
                concurrency=args.concurrency,
                warmup_requests=args.warmup_requests,
                inference_mode=args.inference_mode,
                request_timeout_s=args.request_timeout_s,
                prompt=args.prompt,
                max_tokens=args.max_tokens,
                collect_latency_breakdown=args.collect_latency_breakdown,
            )
            run_results.append(result)
            ttft_str = (
                f" ttft={result['latency_breakdown']['ttft_avg_ms']:.1f}ms"
                if result["latency_breakdown"]["ttft_avg_ms"] is not None
                else ""
            )
            print(
                f"[{scenario.name} run {run_index}] "
                f"success={result['success_count']}/{result['requests_total']} "
                f"tps={result['throughput_rps']:.3f} "
                f"p95={result['latency_p95_ms']:.1f}ms"
                f"{ttft_str}"
            )

    raw_json = run_dir / "raw-runs.json"
    raw_json.write_text(json.dumps(run_results, indent=2), encoding="utf-8")
    raw_csv = run_dir / "raw-requests.csv"
    write_csv(raw_csv, run_results)

    summary = summarize(run_results)
    summary_json = run_dir / "summary.json"
    summary_json.write_text(json.dumps(summary, indent=2), encoding="utf-8")

    meta = {
        "timestamp_utc": now_iso(),
        "git_commit": git_commit(),
        "seed": args.seed,
        "scenarios_json": str(Path(args.scenarios_json)),
        "runs_per_scenario": args.runs_per_scenario,
        "requests_per_run": args.requests_per_run,
        "warmup_requests": args.warmup_requests,
        "concurrency": args.concurrency,
        "request_timeout_s": args.request_timeout_s,
        "inference_mode": args.inference_mode,
        "max_tokens": args.max_tokens,
        "collect_latency_breakdown": args.collect_latency_breakdown,
        "require_speculative_gate": args.require_speculative_gate,
    }
    metadata_json = run_dir / "metadata.json"
    metadata_json.write_text(json.dumps(meta, indent=2), encoding="utf-8")

    report_md = run_dir / "report.md"
    write_markdown_report(report_md, summary, meta)

    manifest = {
        "timestamp_utc": now_iso(),
        "git_commit": meta["git_commit"],
        "artifacts": [
            {"path": p.name, "sha256": sha256_file(p)}
            for p in [raw_json, raw_csv, summary_json, metadata_json, report_md]
        ],
    }
    manifest_json = run_dir / "manifest.json"
    manifest_json.write_text(json.dumps(manifest, indent=2), encoding="utf-8")

    if args.require_speculative_gate:
        violations = speculative_gate_violations(run_results, args.inference_mode)
        if violations:
            print("Speculative activation gate FAILED:")
            for violation in violations:
                print(
                    " - scenario={scenario} run={run_index} base_url={base_url}".format(
                        scenario=violation["scenario"],
                        run_index=violation["run_index"],
                        base_url=violation["base_url"],
                    )
                )
            print(f"Artifacts captured at: {run_dir}")
            raise SystemExit(2)
        print("Speculative activation gate passed.")

    print(f"Wrote benchmark artifacts to: {run_dir}")
    print(f"Summary: {summary_json}")
    print(f"Report: {report_md}")
    print(f"Manifest: {manifest_json}")


if __name__ == "__main__":
    main()
