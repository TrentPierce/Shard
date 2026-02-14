"""OpenAI-compatible local Shard API with SSE streaming.

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
import sys
import time
import uuid
from collections import deque
from collections.abc import AsyncIterator
from typing import Annotated, Any, Literal

import httpx
from fastapi import Depends, FastAPI, Header, HTTPException, Request, Response, status
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import PlainTextResponse, StreamingResponse
from pydantic import BaseModel, Field

from inference import RustControlPlaneClient, cooperative_generate
from golden_ticket import (
    get_generator as get_gt_generator,
    is_scout_banned,
    get_scout_reputation,
    get_all_banned_scouts,
    unban_scout,
    reset_scout_reputation,
)

LOGGER = logging.getLogger("shard.shard_api")

# Try to load BitNet runtime, but allow running without it
# Note: BITNET is checked at runtime in endpoints, not at import time
BITNET = None  # Will be initialized at startup if lib/model available
BitNetRuntime = None  # type: ignore
BitNetConfig = None  # type: ignore

# Create a mock for testing when BITNET is not available
class MockBitNetRuntime:
    def generate(self, prompt, max_tokens):
        return "mock response"
    def tokenize(self, text):
        return [1, 2, 3]
    def rollback(self, pos):
        pass
    def eval_text(self, text):
        return len(text.split())
    def generate_next_token(self, tokens):
        return "test"
    def verify_prefix(self, tokens, draft):
        return draft, None

# Check if we should use mock (for testing or when no real bitnet available)
_use_mock = os.getenv("SHARD_TESTING") == "1" or not (os.getenv("BITNET_LIB") and os.getenv("BITNET_MODEL"))

if _use_mock:
    BITNET = MockBitNetRuntime()
else:
    try:
        from bitnet.ctypes_bridge import BitNetConfig as _BitNetConfig, BitNetRuntime as _BitNetRuntime
        BitNetConfig = _BitNetConfig
        BitNetRuntime = _BitNetRuntime
        try:
            BITNET = _BitNetRuntime()
        except Exception:
            pass
    except Exception:
        pass

# ─── App & Config ────────────────────────────────────────────────────────────

# Define OpenAPI tags for organized endpoint grouping
tags_metadata = [
    {
        "name": "chat",
        "description": (
            "Chat completion endpoints for inference generation. "
            "Supports both streaming and non-streaming modes with OpenAI-compatible API."
        ),
    },
    {
        "name": "scouts",
        "description": (
            "Scout management endpoints for browser Scout nodes. "
            "Manage scout reputation, bans, and task assignments."
        ),
    },
    {
        "name": "system",
        "description": (
            "System management endpoints. "
            "Get network topology, peer list, and health status."
        ),
    },
    {
        "name": "admin",
        "description": (
            "Administrative endpoints. "
            "Scout bans and reputation resets require authentication."
        ),
    },
]

app = FastAPI(
    title="Shard API",
    version="0.4.4",
    description=r"""
    OpenAI-compatible API for the Shard distributed inference network.

    ## Overview

    The Shard API provides server-grade LLM inference through a hybrid
    P2P network. It combines:
    - **Shard nodes** with full models that verify draft tokens
    - **Scout nodes** with draft models that generate token predictions
    - **Distributed inference** for free, unlimited access

    ## Architecture

    ```
    User Request
         ↓
    Scout Nodes (WebLLM) → Draft Token Generation
         ↓
    Shard Nodes (BitNet) → Draft Verification
         ↓
    Final Response
    ```

    ## Features

    ### Inference
    - **Chat Completions**: OpenAI-compatible streaming and non-streaming chat
    - **Distributed Generation**: Hybrid Shard+Scout inference for quality
    - **Golden Ticket Security**: Sybil attack prevention through verification

    ### Network
    - **P2P Networking**: libp2p-based mesh networking
    - **Gossipsub Protocol**: Efficient task distribution
    - **Kademlia DHT**: Peer discovery and routing

    ### Quality
    - **Verify, Don't Trust**: All Scout drafts verified by Shards
    - **Heavier is Truth**: GPU nodes always override browser drafts
    - **Reputation System**: Scout accuracy tracked and scored

    ## Authentication

    When `SHARD_API_KEYS` is configured, include the API key in the header:

    - `Authorization: Bearer <api_key>` OR
    - `X-API-Key: <api_key>`

    See [Authentication Guide](https://shard.network/docs/authentication) for details.

    ## Node Modes

    - **Shard**: Full model host that verifies draft tokens (desktop GPU recommended)
    - **Scout**: Browser node that generates draft tokens (WebGPU recommended)
    - **Leech**: Consumer-only node (lowest priority, queued behind contributors)

    ## API Versioning

    - Current version: v1 (URL path: /api/v1/)
    - Major version changes require consultation with maintainers
    - See [Changelog](CHANGELOG.md) for version history

    ## Production Hardening

    Environment variables:
    - `SHARD_API_KEYS`: Comma-separated valid API keys
    - `SHARD_RATE_LIMIT_PER_MINUTE`: Request limit per client (default: 60)
    - `SHARD_MAX_PROMPT_CHARS`: Maximum prompt length (default: 16000)
    - `SHARD_LOG_LEVEL`: Log level (DEBUG/INFO/WARNING/ERROR)

    ## Rate Limiting

    Requests are rate-limited per client IP:
    - Default: 60 requests per minute
    - Scout endpoints: 120 requests per minute
    - Rate limit headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`

    ## Error Handling

    All errors return JSON with the following structure:

    \`\`\`json
    {
      "error": {
        "message": "Error description",
        "type": "error_type",
        "param": "parameter_name"
      }
    }
    \`\`\`

    See [Error Codes](https://shard.network/docs/errors) for details.
    """,
    docs_url="/docs",
    redoc_url="/redoc",
    openapi_url="/openapi.json",
    openapi_tags=tags_metadata,
    contact={
        "name": "Shard Network",
        "url": "https://github.com/TrentPierce/Shard",
        "email": "contact@shard.network",
    },
    license_info={
        "name": "MIT",
        "url": "https://opensource.org/licenses/MIT",
        "x-logo": {
            "url": "https://shard.network/static/shard-logo.svg",
            "altText": "Shard Logo",
        },
    },
)

# Public API configuration from environment
PUBLIC_API = os.getenv("SHARD_PUBLIC_API", "false").lower() == "true"
PUBLIC_HOST = os.getenv("SHARD_PUBLIC_HOST", "auto-detect")

# CORS: allow any origin for decentralized access
cors_origins = ["*"]
app.add_middleware(
    CORSMiddleware,
    allow_origins=cors_origins,
    allow_methods=["*"],
    allow_headers=["*"],
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
    "golden_tickets_injected": 0,
    "golden_tickets_verified": 0,
    "golden_tickets_failed": 0,
    "scouts_banned": 0,
}

# Initialize Golden Ticket generator singleton
GT_GENERATOR = get_gt_generator()


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
SCOUT_RATE_LIMITER = RateLimiter(int(os.getenv("SHARD_SCOUT_RATE_LIMIT_PER_MINUTE", "120")))


class LatencyProfileStore:
    """In-memory, low-overhead latency samples for local-vs-network comparisons."""

    def __init__(self) -> None:
        self._samples: deque[dict[str, float | int]] = deque(maxlen=1024)
        self._lock = asyncio.Lock()

    async def record_sample(self, sample: dict[str, float | int]) -> None:
        async with self._lock:
            self._samples.append(sample)

    async def summarize(self, p50_ms: float, p90_ms: float, p99_ms: float) -> dict[str, dict[str, float | int]]:
        async with self._lock:
            samples = list(self._samples)

        buckets: dict[str, list[dict[str, float | int]]] = {"p50": [], "p90": [], "p99": []}
        for sample in samples:
            net_ms = float(sample.get("network_rtt_plus_verify_ms", 0.0))
            if net_ms <= p50_ms:
                buckets["p50"].append(sample)
            elif net_ms <= p90_ms:
                buckets["p90"].append(sample)
            else:
                buckets["p99"].append(sample)

        def _avg(rows: list[dict[str, float | int]], key: str) -> float:
            if not rows:
                return 0.0
            return sum(float(r.get(key, 0.0)) for r in rows) / len(rows)

        out: dict[str, dict[str, float | int]] = {}
        for bucket_name, rows in buckets.items():
            out[bucket_name] = {
                "samples": len(rows),
                "avg_tokens": _avg(rows, "tokens"),
                "avg_local_generate_ms": _avg(rows, "local_generate_ms"),
                "avg_network_rtt_plus_verify_ms": _avg(rows, "network_rtt_plus_verify_ms"),
            }
        return out


LATENCY_PROFILE = LatencyProfileStore()


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
    if not lib_path:
        # Auto-discover in current directory
        # check for shard_engine.dll
        local_dll = os.path.join(os.getcwd(), "shard_engine.dll")
        if os.path.exists(local_dll):
            lib_path = local_dll
        else:
            # check in bitnet subfolder just in case
            local_dll = os.path.join(os.getcwd(), "bitnet", "shard_engine.dll")
            if os.path.exists(local_dll):
                lib_path = local_dll

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
    """
    Represents a message in a conversation.

    Messages form the conversation history for chat completions.
    The first message typically has role="system" to set system instructions.
    """
    role: Literal["system", "user", "assistant"]
    content: str = Field(
        ...,
        min_length=1,
        max_length=8000,
        description=(
            "The text content of the message. "
            "System messages define instructions for the model, "
            "user messages are the input prompt, "
            "assistant messages are model responses."
        ),
        examples=[
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Explain quantum computing"},
        ],
    )


class ChatRequest(BaseModel):
    """
    Request for chat completion generation.

    This follows the OpenAI-compatible chat completion API specification.
    Uses distributed inference via Scout nodes and local Shard verification.
    """
    model: str = Field(
        default="shard-hybrid",
        description=(
            "Model identifier to use. Currently supports: "
            "shard-hybrid (hybrid Shard+Scout inference)"
        ),
        examples=["shard-hybrid", "gpt-4", "claude-3"],
    )
    messages: list[Message] = Field(
        ...,
        min_length=1,
        max_length=64,
        description=(
            "Array of message objects containing the conversation history. "
            "Must include at least one message from the user."
        ),
        examples=[
            [
                {"role": "system", "content": "You are a helpful AI assistant."},
                {"role": "user", "content": "What is the capital of France?"},
            ]
        ],
    )
    temperature: float = Field(
        default=0.7,
        ge=0.0,
        le=2.0,
        description=(
            "Sampling temperature for text generation. "
            "Lower values (0.0) produce more deterministic outputs, "
            "higher values (2.0) produce more creative outputs."
        ),
        examples=[0.3, 0.7, 1.0],
    )
    max_tokens: int = Field(
        default=256,
        ge=1,
        le=2048,
        description=(
            "Maximum number of tokens to generate in the response. "
            "Shorter responses are faster and more focused."
        ),
        examples=[64, 256, 512, 1024],
    )
    stream: bool = Field(
        default=False,
        description=(
            "Enable Server-Sent Events (SSE) streaming for real-time token generation. "
            "When true, chunks are sent incrementally rather than all at once."
        ),
        examples=[True, False],
    )


class Choice(BaseModel):
    """
    A single completion choice in a chat completion response.

    Responses may contain multiple choices (n parameter), but this API
    currently returns a single choice with index 0.
    """
    index: int = Field(
        description="Index of this choice (0 for the primary completion)"
    )
    message: dict[str, str] = Field(
        description=(
            "Message content containing the generated response. "
            "Fields: 'role' (assistant), 'content' (text)"
        ),
        examples=[{"role": "assistant", "content": "Paris is the capital of France."}],
    )
    finish_reason: str = Field(
        default="stop",
        description=(
            "Reason why generation stopped. "
            "Values: 'stop' (normal stop), 'length' (max tokens reached)"
        ),
        examples=["stop", "length"],
    )


class ChatResponse(BaseModel):
    """
    Chat completion response matching OpenAI API specification.
    """
    id: str = Field(
        description="Unique identifier for this completion (e.g., 'chatcmpl-abc123')",
        examples=["chatcmpl-abc123def456"],
    )
    object: str = Field(
        default="chat.completion",
        description="Object type, always 'chat.completion'",
    )
    choices: list[Choice] = Field(
        description="Array of generated completion choices",
        examples=[[{"index": 0, "message": {"role": "assistant", "content": "Hello!"}, "finish_reason": "stop"}]],
    )
    usage: dict[str, int] = Field(
        description="Token usage statistics for this request",
        examples=[
            {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15,
            }
        ],
    )


# ─── Local Model + Verification ─────────────────────────────────────────────



_session_eval_pos: dict[str, int] = {}

async def _local_generate(generated: list[str], prompt: str, request_id: str) -> str | None:
    runtime = await get_or_load_bitnet()
    if runtime is None:
        return None

    pos = _session_eval_pos.get(request_id, 0)
    if pos == 0:
        # Reset engine state for new request and eval prompt
        runtime.rollback(999999) 
        pos = runtime.eval_text(prompt)
        _session_eval_pos[request_id] = pos

    # Eval any new tokens in generated
    gen_idx = _session_eval_pos.get(f"{request_id}_idx", 0)
    while gen_idx < len(generated):
        runtime.eval_text(generated[gen_idx])
        gen_idx += 1
    _session_eval_pos[f"{request_id}_idx"] = gen_idx

    try:
        return runtime.generate_next_token(generated)
    except Exception:
        # Skip logger to avoid missing import issues in script
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


@app.get(
    "/health",
    tags=["system"],
    summary="Health check endpoint",
    description=(
        "Returns the health status of the API and its dependencies. "
        "Use this for monitoring and load balancer health checks."
    ),
)
async def health() -> dict[str, Any]:
    client = _get_http_client()
    rust_status = "unreachable"
    try:
        r = await client.get("/health")
        if r.status_code == 200:
            rust_status = "connected"
    except httpx.HTTPError as exc:
        LOGGER.warning("Rust sidecar health check failed: %s", exc)

    # Check if BitNet is loaded (don't try to load, just check)
    bitnet_loaded = BITNET is not None
    
    return {
        "status": "ok",
        "idle": STATE.is_idle,
        "accepting_swarm_jobs": STATE.is_idle,
        "rust_sidecar": rust_status,
        "rust_url": RUST_URL,
        "bitnet_loaded": bitnet_loaded,
        "bitnet_lib": os.getenv("BITNET_LIB", ""),
        "bitnet_model": os.getenv("BITNET_MODEL", ""),
        "cors_origins": cors_origins,
    }


@app.get(
    "/v1/system/topology",
    tags=["system"],
    summary="Get network topology",
    description=(
        "Retrieves the network topology for browser Scout auto-discovery. "
        "Scouts use this endpoint to find Shard nodes to connect to."
    ),
)
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
        "shard_webrtc_multiaddr": None,
        "shard_ws_multiaddr": None,
        "detail": "Rust sidecar not reachable",
    }


@app.get(
    "/v1/system/peers",
    tags=["system"],
    summary="List connected peers",
    description=(
        "Returns information about currently connected peers in the network. "
        "Use for monitoring and debugging."
    ),
)
async def system_peers() -> dict[str, Any]:
    client = _get_http_client()
    try:
        r = await client.get("/peers")
        if r.status_code == 200:
            return r.json()
    except httpx.HTTPError as exc:
        LOGGER.warning("Peers fetch failed: %s", exc)
    return {"peers": [], "count": 0}


@app.post(
    "/v1/chat/completions",
    tags=["chat"],
    summary="Create chat completion",
    description=(
        "Creates a model response for the given chat conversation. "
        "Uses distributed inference with Scout nodes and Shard verification. "
        "Supports both streaming and non-streaming modes."
    ),
    responses={
        200: {
            "description": "Chat completion created successfully",
            "content": {
                "application/json": {
                    "example": {
                        "id": "chatcmpl-abc123def456",
                        "object": "chat.completion",
                        "created": 1704062400,
                        "model": "shard-hybrid",
                        "choices": [
                            {
                                "index": 0,
                                "message": {"role": "assistant", "content": "Paris is the capital of France."},
                                "finish_reason": "stop",
                            }
                        ],
                        "usage": {
                            "prompt_tokens": 10,
                            "completion_tokens": 5,
                            "total_tokens": 15,
                        },
                    }
                }
            },
        },
        429: {
            "description": "Rate limit exceeded",
            "content": {
                "application/json": {
                    "example": {
                        "detail": "Rate limit exceeded",
                        "X-RateLimit-Limit": "60",
                        "X-RateLimit-Remaining": "0",
                    }
                }
            },
        },
    },
)
async def chat_completions(
    payload: ChatRequest,
    request: Request,
    principal: str = Depends(require_api_key),
) -> Any:
    STATE.last_local_activity_ts = time.time()
    METRICS["chat_requests_total"] += 1
    await enforce_rate_limit(request, principal)

    # Check if BitNet model is loaded - return clear error if not
    if BITNET is None:
        bitnet_lib = os.getenv("BITNET_LIB", "")
        bitnet_model = os.getenv("BITNET_MODEL", "")
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail=(
                "BitNet model not loaded. Please set BITNET_LIB and BITNET_MODEL "
                "environment variables and restart the server. "
                f"Current: BITNET_LIB='{bitnet_lib}', BITNET_MODEL='{bitnet_model}'"
            ),
        )

    prompt_parts: list[str] = ["<|begin_of_text|>"]
    for m in payload.messages:
        prompt_parts.append(f"<|start_header_id|>{m.role}<|end_header_id|>\n\n{m.content}<|eot_id|>")
    prompt_parts.append("<|start_header_id|>assistant<|end_header_id|>\n\n")
    user_text = "".join(prompt_parts)

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
            telemetry_hook=LATENCY_PROFILE.record_sample,
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
    
    # Check if BitNet model is loaded
    if BITNET is None:
        bitnet_lib = os.getenv("BITNET_LIB", "")
        bitnet_model = os.getenv("BITNET_MODEL", "")
        error = {
            "error": {
                "message": (
                    f"BitNet model not loaded. Please set BITNET_LIB and BITNET_MODEL "
                    f"environment variables. Current: BITNET_LIB='{bitnet_lib}', BITNET_MODEL='{bitnet_model}'"
                ),
                "type": "model_not_loaded"
            }
        }
        yield f"data: {json.dumps(error)}\n\n"
        yield "data: [DONE]\n\n"
        return
    
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
            telemetry_hook=LATENCY_PROFILE.record_sample,
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


@app.get(
    "/v1/metrics/latency_profile",
    tags=["system"],
    summary="Latency profile for local vs P2P speculative path",
    description=(
        "Compares local generation time for N tokens against network RTT + verification "
        "for scout drafts, grouped by current P2P latency percentiles."
    ),
)
async def latency_profile() -> dict[str, Any]:
    client = _get_http_client()
    p2p = {"p50": 0.0, "p90": 0.0, "p99": 0.0, "samples": 0}
    try:
        resp = await client.get("/metrics/latency-profile")
        if resp.status_code == 200:
            payload = resp.json().get("gossipsub_propagation_ms", {})
            p2p = {
                "p50": float(payload.get("p50", 0.0)),
                "p90": float(payload.get("p90", 0.0)),
                "p99": float(payload.get("p99", 0.0)),
                "samples": int(payload.get("samples", 0)),
            }
    except httpx.HTTPError as exc:
        LOGGER.warning("Rust latency profile fetch failed: %s", exc)

    summary = await LATENCY_PROFILE.summarize(p2p["p50"], p2p["p90"], p2p["p99"])
    return {
        "status": "ok",
        "p2p_latency_ms": p2p,
        "local_vs_network": summary,
    }


@app.get(
    "/metrics",
    tags=["system"],
    summary="Prometheus metrics",
    description=(
        "Returns Prometheus-style plaintext metrics for monitoring. "
        "Format: `# HELP <metric_name> <help_text>` and `# TYPE <metric_name> <type>`"
    ),
)
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
        "# HELP shard_golden_tickets_injected_total Total Golden Tickets injected",
        "# TYPE shard_golden_tickets_injected_total counter",
        f"shard_golden_tickets_injected_total {METRICS['golden_tickets_injected']}",
        "# HELP shard_golden_tickets_verified_total Total Golden Tickets verified",
        "# TYPE shard_golden_tickets_verified_total counter",
        f"shard_golden_tickets_verified_total {METRICS['golden_tickets_verified']}",
        "# HELP shard_golden_tickets_failed_total Total Golden Tickets failed",
        "# TYPE shard_golden_tickets_failed_total counter",
        f"shard_golden_tickets_failed_total {METRICS['golden_tickets_failed']}",
        "# HELP shard_scouts_banned_total Total scouts banned",
        "# TYPE shard_scouts_banned_total counter",
        f"shard_scouts_banned_total {METRICS['scouts_banned']}",
    ]
    return "\n".join(lines) + "\n"


# ─── Golden Ticket & Reputation Endpoints ────────────────────────────────────


@app.get(
    "/v1/scout/reputation/{peer_id}",
    tags=["scouts"],
    summary="Get scout reputation",
    description="Retrieves reputation information for a specific scout node.",
)
async def get_reputation(
    peer_id: str,
    _principal: str = Depends(require_api_key),
) -> dict[str, object]:
    """Get reputation information for a specific scout."""
    return get_scout_reputation(peer_id)


@app.get(
    "/v1/scout/banned",
    tags=["scouts"],
    summary="List banned scouts",
    description="Returns a list of all currently banned scout peer IDs.",
)
async def list_banned_scouts(
    _principal: str = Depends(require_api_key),
) -> dict[str, object]:
    """List all currently banned scouts."""
    return {
        "banned_scouts": get_all_banned_scouts(),
        "count": len(get_all_banned_scouts()),
    }


@app.post(
    "/v1/scout/unban/{peer_id}",
    tags=["admin"],
    summary="Unban a scout (admin override)",
    description=(
        "Manually unbans a scout node that has been banned from the network. "
        "Requires authentication. Only use for legitimate recovery scenarios."
    ),
    responses={
        200: {
            "description": "Scout unbanned successfully",
            "content": {
                "application/json": {
                    "example": {
                        "peer_id": "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
                        "unbanned": True,
                        "detail": "Scout unbanned",
                    }
                }
            },
        }
    },
)
async def admin_unban_scout(
    peer_id: str,
    _principal: str = Depends(require_api_key),
) -> dict[str, object]:
    """Manually unban a scout (admin override)."""
    success = unban_scout(peer_id)
    return {
        "peer_id": peer_id,
        "unbanned": success,
        "detail": "Scout unbanned" if success else "Scout not found in ban list",
    }


@app.post("/v1/scout/reset-reputation/{peer_id}")
async def admin_reset_reputation(
    peer_id: str,
    _principal: str = Depends(require_api_key),
) -> dict[str, object]:
    """Reset a scout's reputation (admin override)."""
    success = reset_scout_reputation(peer_id)
    return {
        "peer_id": peer_id,
        "reset": success,
        "detail": "Reputation reset" if success else "Scout not found",
    }


@app.post(
    "/v1/scout/draft",
    tags=["scouts"],
    summary="Submit scout draft tokens",
    description=(
        "Accepts draft generation results from Scout nodes. "
        "If the work is a Golden Ticket, the response is verified for correctness. "
        "Rate limited to prevent spam."
    ),
)
async def submit_scout_draft(
    request: Request,
) -> dict[str, object]:
    """Submit draft tokens from a scout for verification.
    
    This endpoint accepts draft generation results from Scout nodes.
    If the work is a Golden Ticket, the response is verified for correctness.
    
    Rate limited to prevent spam from malicious scouts.
    """
    client_ip = request.client.host if request.client else "unknown"
    allowed, remaining = await SCOUT_RATE_LIMITER.check(client_ip)
    if not allowed:
        METRICS["rate_limited_total"] += 1
        raise HTTPException(
            status_code=status.HTTP_429_TOO_MANY_REQUESTS,
            detail=f"Rate limit exceeded. Try again in {remaining}s",
            headers={"Retry-After": str(remaining)},
        )
    
    try:
        data = await request.json()
    except Exception:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid JSON payload",
        )
    
    work_id = data.get("workId") or data.get("request_id")
    scout_id = data.get("scoutId") or data.get("scout_id")
    draft_text = data.get("draftText") or data.get("draft_text") or ""
    
    if not work_id or not scout_id:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Missing workId or scoutId",
        )
    
    # Check if scout is banned
    if is_scout_banned(scout_id):
        LOGGER.warning("Rejected draft from banned scout: %s", scout_id)
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Scout is banned from the network",
        )
    
    # Check if this is a Golden Ticket verification
    gt_result = GT_GENERATOR.verify_response(
        request_id=work_id,
        scout_peer_id=scout_id,
        scout_response=draft_text,
    )
    
    if gt_result is True:
        # Golden Ticket verified successfully
        METRICS["golden_tickets_verified"] += 1
        LOGGER.info("Golden Ticket verified: scout=%s work=%s", scout_id, work_id)
        return {
            "success": True,
            "detail": "Golden Ticket verified successfully",
            "verified": True,
        }
    elif gt_result is False:
        # Golden Ticket failed
        METRICS["golden_tickets_failed"] += 1
        METRICS["scouts_banned"] = len(get_all_banned_scouts())
        LOGGER.warning("Golden Ticket FAILED: scout=%s work=%s", scout_id, work_id)
        return {
            "success": False,
            "detail": "Golden Ticket verification failed",
            "verified": False,
        }
    
    # Not a Golden Ticket - normal draft submission
    # Forward to Rust sidecar via control plane
    try:
        control = RustControlPlaneClient(base_url=RUST_URL)
        # Store the result for the cooperative_generate loop to pick up
        await control.submit_draft_result(work_id, scout_id, draft_text)
        await control.close()
        
        return {
            "success": True,
            "detail": "Draft submitted for verification",
            "verified": None,
        }
    except Exception as exc:
        LOGGER.exception("Failed to submit draft to control plane")
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail=f"Failed to process draft: {exc}",
        )


@app.get(
    "/v1/scout/work",
    tags=["scouts"],
    summary="Get work for scouts",
    description=(
        "Retrieves work assignments for scouts to process. "
        "May include Golden Ticket verification tasks. "
        "Returns 204 No Content if no work is available."
    ),
)
async def get_scout_work(
    request: Request,
) -> Response:
    """Get work for a scout to process.
    
    Returns a work request with potentially injected Golden Ticket.
    
    Rate limited to prevent spam from malicious scouts.
    """
    client_ip = request.client.host if request.client else "unknown"
    allowed, remaining = await SCOUT_RATE_LIMITER.check(client_ip)
    if not allowed:
        METRICS["rate_limited_total"] += 1
        raise HTTPException(
            status_code=status.HTTP_429_TOO_MANY_REQUESTS,
            detail=f"Rate limit exceeded. Try again in {remaining}s",
            headers={"Retry-After": str(remaining)},
        )
    
    # This would typically come from the Rust sidecar work queue
    # For now, return 204 No Content to indicate no work available
    return Response(status_code=status.HTTP_204_NO_CONTENT)


@app.get(
    "/v1/models",
    tags=["chat"],
    summary="List available models",
    description=(
        "Returns a list of available models for use with the API. "
        "Use the 'shard-hybrid' model for hybrid inference."
    ),
)
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



# ─── Static Files (Must be last) ──────────────────────────────────────────────────

# Auto-serve static files if bundled
if getattr(sys, "frozen", False):
    base_dir = os.path.dirname(sys.executable)
    # In onedir mode, resources are in _internal/web relative to the exe
    static_dir = os.path.join(base_dir, "_internal", "web")
    
    if os.path.exists(static_dir):
        LOGGER.info(f"Mounting static files from {static_dir}")
        app.mount("/", StaticFiles(directory=static_dir, html=True), name="static")
    else:
        LOGGER.warning(f"Static web directory not found at {static_dir}")
