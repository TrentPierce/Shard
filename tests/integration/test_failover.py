from __future__ import annotations

import asyncio
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import httpx
import pytest


ROOT = Path(__file__).resolve().parents[2]
RUST_DIR = ROOT / "desktop" / "rust"
BIN = RUST_DIR / "target" / "debug" / ("shard-daemon.exe" if os.name == "nt" else "shard-daemon")


def _spawn_daemon(control_port: int, tcp_port: int, data_dir: Path) -> subprocess.Popen[str]:
    env = os.environ.copy()
    env["SHARD_DATA_DIR"] = str(data_dir)
    env["RUST_LOG"] = env.get("RUST_LOG", "info")
    env["SHARD_REQUIRE_ENGINE_FOR_CONTRIBUTE"] = "0"
    cmd = [
        str(BIN),
        "--control-port",
        str(control_port),
        "--telemetry-ws-port",
        str(9400 + (control_port - 9091)),
        "--tcp-port",
        str(tcp_port),
        "--webrtc-port",
        str(9200 + (control_port - 9091)),
        "--quic-port",
        str(9300 + (control_port - 9091)),
        "--ha-mode",
    ]
    return subprocess.Popen(
        cmd,
        cwd=str(RUST_DIR),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


async def _wait_health(base_url: str, timeout_seconds: float = 30.0) -> None:
    deadline = time.monotonic() + timeout_seconds
    async with httpx.AsyncClient(timeout=1.5) as client:
        while time.monotonic() < deadline:
            try:
                response = await client.get(f"{base_url}/health")
                if response.status_code == 200 and response.json().get("status") in {"ok", "degraded"}:
                    return
            except Exception:
                pass
            await asyncio.sleep(0.5)
    raise AssertionError(f"Health check timeout: {base_url}")


async def _get_role(base_url: str) -> dict:
    async with httpx.AsyncClient(timeout=2.0) as client:
        response = await client.get(f"{base_url}/node/consensus-role")
        response.raise_for_status()
        return response.json()


async def _find_leader(urls: list[str], timeout_seconds: float = 10.0) -> str:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        for url in urls:
            try:
                role = await _get_role(url)
                if role.get("role") == "leader":
                    return url
            except Exception:
                continue
        await asyncio.sleep(0.3)
    raise AssertionError("No leader elected within timeout")


async def _chat_once(client: httpx.AsyncClient, url: str, idx: int) -> str:
    payload = {
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": f"hello {idx}"}],
        "stream": False,
        "max_tokens": 16,
    }
    response = await client.post(f"{url}/v1/chat/completions", json=payload)
    response.raise_for_status()
    body = response.json()
    return str(body.get("id", f"fallback-{idx}"))


@pytest.mark.asyncio
async def test_failover():
    if not BIN.exists():
        pytest.skip(f"binary not built: {BIN}")

    ports = [(19191, 14001), (19192, 14002), (19193, 14003)]
    processes: list[subprocess.Popen[str]] = []
    tmp_dirs: list[tempfile.TemporaryDirectory[str]] = []

    try:
        for control_port, tcp_port in ports:
            td = tempfile.TemporaryDirectory(prefix=f"shard-ha-{control_port}-")
            tmp_dirs.append(td)
            proc = _spawn_daemon(control_port, tcp_port, Path(td.name))
            processes.append(proc)

        urls = [f"http://127.0.0.1:{control}" for control, _ in ports]
        await asyncio.gather(*[_wait_health(url) for url in urls])

        leader_url = await _find_leader(urls, timeout_seconds=20.0)

        results: list[str] = []
        started = 0
        async with httpx.AsyncClient(timeout=15.0) as client:
            for i in range(50):
                rid = await _chat_once(client, leader_url, i)
                results.append(rid)
                started += 1

            leader_index = urls.index(leader_url)
            processes[leader_index].terminate()

            new_leader = await _find_leader([u for u in urls if u != leader_url], timeout_seconds=10.0)
            for i in range(50, 100):
                attempts = [new_leader] + [u for u in urls if u not in {leader_url, new_leader}]
                sent = False
                for attempt_url in attempts:
                    try:
                        rid = await _chat_once(client, attempt_url, i)
                        results.append(rid)
                        sent = True
                        break
                    except Exception:
                        await asyncio.sleep(0.1)
                if not sent:
                    raise AssertionError(f"request {i} failed after retries")

        assert len(results) == 100, "all 100 requests must complete"
        assert len(set(results)) == len(results), "response IDs should be unique"
    finally:
        for proc in processes:
            if proc.poll() is None:
                proc.terminate()
        for proc in processes:
            try:
                proc.wait(timeout=5)
            except Exception:
                proc.kill()
        for td in tmp_dirs:
            td.cleanup()
