"""OpenAI-compatible local Oracle API with SSE streaming.

Service roles:
- Driver API in Python (OpenAI-compatible, with streaming)
- Sidecar networking daemon in Rust (libp2p) — communicated via HTTP
- In-process bitnet runtime via ctypes bridge
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
import time
import uuid
from collections.abc import AsyncIterator
from typing import Annotated, Any, Literal

import httpx
from fastapi import Depends, FastAPI, Header, HTTPException, Request, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import PlainTextResponse, StreamingResponse
from pydantic import BaseModel, Field

from bitnet.ctypes_bridge import BitNetConfig, BitNetRuntime
from inference import RustControlPlaneClient, cooperative_generate

LOGGER = logging.getLogger("shard.oracle_api")

# ─── App & Config ────────────────────────────────────────────────────────────

app = FastAPI(title="Shard Oracle API", version="0.4.0")

cors_origins = [o.strip() for o in os.getenv("SHARD_CORS_ORIGINS", "http://localhost:3000,http://127.0.0.1:3000").split(",") if o.strip()]
app.add_middleware(
    CORSMiddleware,
    allow_origins=cors_origins,
    allow_methods=["GET", "POST", "OPTIONS"],
    allow_headers=["Content-Type", "Authorization"],
)

RUST_URL = os.getenv("SHARD_RUST_URL", "http://127.0.0.1:9091")

# ─── State ───────────────────────────────────────────────────────────────────


class NodeState:
    def __init__(self) -> None:
        self.last_local_activity_ts: float = time.time()
        self.idle_after_seconds: int = 30

    @property
    def is_idle(self) -> bool:
        return (time.time() - self.last_local_activity_ts) >= self.idle_after_seconds


STATE = NodeState()
BITNET: BitNetRuntime | None = None
_http_client: httpx.AsyncClient | None = None
_bitnet_lock = asyncio.Lock()

API_KEYS = {k.strip() for k in os.getenv("SHARD_API_KEYS", "").split(",") if k.strip()}
RATE_LIMIT_PER_MINUTE = int(os.getenv("SHARD_RATE_LIMIT_PER_MINUTE", "60"))
MAX_PROMPT_CHARS = int(os.getenv("SHARD_MAX_PROMPT_CHARS", "16000"))

METRICS: dict[str, int] = {
    "chat_requests_total": 0,
    "chat_failures_total": 0,
    "auth_failures_total": 0,
    "rate_limited_total": 0,
}


class RateLimiter:
    """Simple fixed-window limiter for local deployments."""

    def __init__(self, limit_per_minute: int) -> None:
        self.limit_per_minute = max(1, limit_per_minute)
        self._buckets: dict[str, tuple[int, int]] = {}
        self._lock = asyncio.Lock()

    async def check(self, key: str) -> tuple[bool, int]:
        now_window = int(time.time() // 60)
        async with self._lock:
            count, window = self._buckets.get(key, (0, now_window))
            if window != now_window:
                count = 0
                window = now_window

            if count >= self.limit_per_minute:
                return False, 0

            count += 1
            self._buckets[key] = (count, window)
            remaining = self.limit_per_minute - count
            return True, remaining


RATE_LIMITER = RateLimiter(RATE_LIMIT_PER_MINUTE)


def _get_http_client() -> httpx.AsyncClient:
    global _http_client
    if _http_client is None:
        _http_client = httpx.AsyncClient(base_url=RUST_URL, timeout=5.0)
    return _http_client


async def get_or_load_bitnet() -> BitNetRuntime | None:
    global BITNET
    if BITNET is not None:
        return BITNET

    lib_path = os.getenv("BITNET_LIB")
    model_path = os.getenv("BITNET_MODEL")
    if not lib_path or not model_path:
        return None

    async with _bitnet_lock:
        if BITNET is not None:
            return BITNET
        try:
            runtime = await asyncio.to_thread(
                BitNetRuntime,
                BitNetConfig(lib_path=lib_path, model_path=model_path),
            )
            BITNET = runtime
            LOGGER.info("Loaded BitNet runtime from %s using model %s", lib_path, model_path)
            return BITNET
        except Exception:
            LOGGER.exception("Failed to load BitNet runtime")
            BITNET = None
            return None


def _auth_error() -> HTTPException:
    return HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Missing or invalid API key",
    )


async def require_api_key(
    authorization: Annotated[str | None, Header()] = None,
    x_api_key: Annotated[str | None, Header(alias="X-API-Key")] = None,
) -> str:
    """Optional-by-default API key auth. Enabled when SHARD_API_KEYS is set."""
    if not API_KEYS:
        return "anonymous"

    bearer = None
    if authorization and authorization.lower().startswith("bearer "):
        bearer = authorization.split(" ", 1)[1].strip()

    candidate = x_api_key or bearer
    if not candidate or candidate not in API_KEYS:
        METRICS["auth_failures_total"] += 1
        raise _auth_error()

    return candidate


async def enforce_rate_limit(request: Request, principal: str) -> None:
    identity = principal
    if identity == "anonymous":
        identity = request.client.host if request.client else "unknown"

    allowed, remaining = await RATE_LIMITER.check(identity)
    if not allowed:
        METRICS["rate_limited_total"] += 1
        raise HTTPException(
            status_code=status.HTTP_429_TOO_MANY_REQUESTS,
            detail="Rate limit exceeded",
            headers={"X-RateLimit-Limit": str(RATE_LIMIT_PER_MINUTE), "X-RateLimit-Remaining": "0"},
        )

    request.state.rate_limit_remaining = remaining


# ─── Models ──────────────────────────────────────────────────────────────────


class Message(BaseModel):
    role: Literal["system", "user", "assistant"]
    content: str = Field(min_length=1, max_length=8000)


class ChatRequest(BaseModel):
    model: str = Field(default="shard-hybrid")
    messages: list[Message] = Field(min_length=1, max_length=64)
    temperature: float = Field(default=0.7, ge=0.0, le=2.0)
    max_tokens: int = Field(default=256, ge=1, le=2048)
    stream: bool = Field(default=False)


class Choice(BaseModel):
    index: int
    message: dict[str, str]
    finish_reason: str = "stop"


class ChatResponse(BaseModel):
    id: str
    object: str = "chat.completion"
    choices: list[Choice]
    usage: dict[str, int]


# ─── Local Model + Verification ─────────────────────────────────────────────


async def _local_generate(generated: list[str], prompt: str) -> str | None:
    runtime = await get_or_load_bitnet()
    if runtime is None:
        return None

    if not generated and prompt:
        # Prime deterministic token map with prompt words for stable decoding.
        runtime.encode_text(prompt)

    try:
        return runtime.generate_next_token(generated)
    except Exception:
        LOGGER.exception("Local token generation failed")
        raise


async def _verify_draft(generated: list[str], draft: list[str]) -> tuple[list[str], str | None]:
    runtime = await get_or_load_bitnet()
    if runtime is None:
        # No runtime loaded: deterministic fallback is strict reject.
        return [], None
    try:
        return runtime.verify_prefix(generated, draft)
    except Exception:
        LOGGER.exception("Draft verification failed")
        return [], None


# ─── Endpoints ───────────────────────────────────────────────────────────────


@app.get("/health")
async def health() -> dict[str, Any]:
    client = _get_http_client()
    rust_status = "unreachable"
    try:
        r = await client.get("/health")
        if r.status_code == 200:
            rust_status = "connected"
    except httpx.HTTPError as exc:
        LOGGER.warning("Rust sidecar health check failed: %s", exc)

    return {
        "status": "ok",
        "idle": STATE.is_idle,
        "accepting_swarm_jobs": STATE.is_idle,
        "rust_sidecar": rust_status,
        "rust_url": RUST_URL,
        "bitnet_loaded": BITNET is not None,
        "cors_origins": cors_origins,
    }


@app.get("/v1/system/topology")
async def system_topology() -> dict[str, Any]:
    client = _get_http_client()
    try:
        r = await client.get("/topology")
        if r.status_code == 200:
            data = r.json()
            return {"status": "ok", "source": "rust-sidecar", **data}
    except httpx.HTTPError as exc:
        LOGGER.warning("Topology fetch failed: %s", exc)
    return {
        "status": "degraded",
        "source": "fallback",
        "oracle_webrtc_multiaddr": None,
        "oracle_ws_multiaddr": None,
        "detail": "Rust sidecar not reachable",
    }


@app.get("/v1/system/peers")
async def system_peers() -> dict[str, Any]:
    client = _get_http_client()
    try:
        r = await client.get("/peers")
        if r.status_code == 200:
            return r.json()
    except httpx.HTTPError as exc:
        LOGGER.warning("Peers fetch failed: %s", exc)
    return {"peers": [], "count": 0}


@app.post("/v1/chat/completions")
async def chat_completions(
    payload: ChatRequest,
    request: Request,
    principal: str = Depends(require_api_key),
) -> Any:
    STATE.last_local_activity_ts = time.time()
    METRICS["chat_requests_total"] += 1
    await enforce_rate_limit(request, principal)

    user_text = "\n".join(m.content for m in payload.messages if m.role == "user")
    if len(user_text) > MAX_PROMPT_CHARS:
        raise HTTPException(
            status_code=status.HTTP_413_REQUEST_ENTITY_TOO_LARGE,
            detail=f"Prompt too large (>{MAX_PROMPT_CHARS} chars)",
        )
    completion_id = f"chatcmpl-{uuid.uuid4().hex[:12]}"

    # ── streaming ──
    if payload.stream:
        return StreamingResponse(
            _stream_generate(completion_id, user_text, payload.max_tokens),
            media_type="text/event-stream",
            headers={
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
                "X-Accel-Buffering": "no",
            },
        )

    # ── non-streaming ──
    try:
        control = RustControlPlaneClient(base_url=RUST_URL)
    except RuntimeError as exc:
        raise HTTPException(status_code=status.HTTP_503_SERVICE_UNAVAILABLE, detail=str(exc)) from exc

    tokens: list[str] = []
    try:
        async for tok in cooperative_generate(
            prompt=user_text,
            local_model_generate=_local_generate,
            verify_draft=_verify_draft,
            control_plane=control,
            max_tokens=payload.max_tokens,
        ):
            tokens.append(tok)
    except Exception as exc:
        METRICS["chat_failures_total"] += 1
        LOGGER.exception("Non-streaming inference failed")
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail=f"Inference failed: {exc}",
        ) from exc
    finally:
        await control.close()

    content = " ".join(tokens)
    return ChatResponse(
        id=completion_id,
        choices=[Choice(index=0, message={"role": "assistant", "content": content})],
        usage={
            "prompt_tokens": len(user_text.split()),
            "completion_tokens": len(tokens),
            "total_tokens": len(user_text.split()) + len(tokens),
        },
    )


async def _stream_generate(
    completion_id: str,
    prompt: str,
    max_tokens: int,
) -> AsyncIterator[str]:
    """SSE stream of chat completion chunks (OpenAI-compatible)."""
    try:
        control = RustControlPlaneClient(base_url=RUST_URL)
    except RuntimeError as exc:
        error = {"error": {"message": str(exc), "type": "service_unavailable"}}
        yield f"data: {json.dumps(error)}\n\n"
        yield "data: [DONE]\n\n"
        return

    try:
        async for token in cooperative_generate(
            prompt=prompt,
            local_model_generate=_local_generate,
            verify_draft=_verify_draft,
            control_plane=control,
            max_tokens=max_tokens,
        ):
            chunk = {
                "id": completion_id,
                "object": "chat.completion.chunk",
                "created": int(time.time()),
                "model": "shard-hybrid",
                "choices": [
                    {
                        "index": 0,
                        "delta": {"content": token + " "},
                        "finish_reason": None,
                    }
                ],
            }
            yield f"data: {json.dumps(chunk)}\n\n"
            await asyncio.sleep(0.005)
    except Exception as exc:
        METRICS["chat_failures_total"] += 1
        LOGGER.exception("Streaming inference failed")
        error = {"error": {"message": f"Inference failed: {exc}", "type": "inference_error"}}
        yield f"data: {json.dumps(error)}\n\n"
    finally:
        await control.close()

    # Final chunk
    final = {
        "id": completion_id,
        "object": "chat.completion.chunk",
        "created": int(time.time()),
        "model": "shard-hybrid",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
    }
    yield f"data: {json.dumps(final)}\n\n"
    yield "data: [DONE]\n\n"


@app.get("/metrics", response_class=PlainTextResponse)
async def metrics() -> str:
    """Prometheus-style plaintext counters for lightweight monitoring."""
    lines = [
        "# HELP shard_chat_requests_total Total chat completion requests",
        "# TYPE shard_chat_requests_total counter",
        f"shard_chat_requests_total {METRICS['chat_requests_total']}",
        "# HELP shard_chat_failures_total Total inference failures",
        "# TYPE shard_chat_failures_total counter",
        f"shard_chat_failures_total {METRICS['chat_failures_total']}",
        "# HELP shard_auth_failures_total Total authentication failures",
        "# TYPE shard_auth_failures_total counter",
        f"shard_auth_failures_total {METRICS['auth_failures_total']}",
        "# HELP shard_rate_limited_total Total rate-limited requests",
        "# TYPE shard_rate_limited_total counter",
        f"shard_rate_limited_total {METRICS['rate_limited_total']}",
    ]
    return "\n".join(lines) + "\n"


@app.get("/v1/models")
async def list_models() -> dict[str, Any]:
    return {
        "object": "list",
        "data": [
            {
                "id": "shard-hybrid",
                "object": "model",
                "owned_by": "shard-network",
                "permission": [],
            }
        ],
    }
