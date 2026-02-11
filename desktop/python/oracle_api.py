"""OpenAI-compatible local Oracle API with SSE streaming.

Service roles:
- Driver API in Python (OpenAI-compatible, with streaming)
- Sidecar networking daemon in Rust (libp2p) — communicated via HTTP
- In-process bitnet runtime via ctypes bridge
"""

from __future__ import annotations

import asyncio
import json
import os
import time
import uuid
from collections.abc import AsyncIterator
from typing import Any

import httpx
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field

from bitnet.ctypes_bridge import BitNetConfig, BitNetRuntime
from inference import cooperative_generate, RustControlPlaneClient

# ─── App & Config ────────────────────────────────────────────────────────────

app = FastAPI(title="Shard Oracle API", version="0.3.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
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
BITNET: BitNetRuntime | None = None
_http_client: httpx.AsyncClient | None = None


def _get_http_client() -> httpx.AsyncClient:
    global _http_client
    if _http_client is None:
        _http_client = httpx.AsyncClient(base_url=RUST_URL, timeout=5.0)
    return _http_client


def maybe_load_bitnet() -> BitNetRuntime | None:
    global BITNET
    if BITNET is not None:
        return BITNET

    lib_path = os.getenv("BITNET_LIB")
    model_path = os.getenv("BITNET_MODEL")
    if not lib_path or not model_path:
        return None

    BITNET = BitNetRuntime(BitNetConfig(lib_path=lib_path, model_path=model_path))
    return BITNET


# ─── Models ──────────────────────────────────────────────────────────────────

class Message(BaseModel):
    role: str
    content: str


class ChatRequest(BaseModel):
    model: str = Field(default="shard-hybrid")
    messages: list[Message]
    temperature: float = Field(default=0.7)
    max_tokens: int = Field(default=256)
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


# ─── Stub Local Model ───────────────────────────────────────────────────────

async def _stub_local_generate(generated: list[str], prompt: str) -> str | None:
    """Placeholder local model generation.

    When BitNet is loaded, this would call the real model.
    For now it produces a finite scaffold response.
    """
    runtime = maybe_load_bitnet()
    if runtime is not None:
        # Real model — would call runtime.generate_next_token(...)
        # For now still a scaffold since the C ABI only exposes verify_prefix
        pass

    scaffold_tokens = (
        "This is a scaffold response from the Shard hybrid inference network. "
        "Once BitNet is loaded and Scout peers are connected, this will produce "
        "real model output verified through speculative decoding."
    ).split()

    idx = len(generated)
    if idx < len(scaffold_tokens):
        return scaffold_tokens[idx]
    return None


async def _stub_verify(generated: list[str], draft: list[str]) -> tuple[list[str], str | None]:
    """Placeholder draft verification. Accepts all tokens for now."""
    return draft, None


# ─── Endpoints ───────────────────────────────────────────────────────────────

@app.get("/health")
async def health() -> dict[str, Any]:
    client = _get_http_client()
    rust_status = "unreachable"
    try:
        r = await client.get("/health")
        if r.status_code == 200:
            rust_status = "connected"
    except Exception:
        pass

    return {
        "status": "ok",
        "idle": STATE.is_idle,
        "accepting_swarm_jobs": STATE.is_idle,
        "rust_sidecar": rust_status,
        "rust_url": RUST_URL,
        "bitnet_loaded": BITNET is not None,
    }


@app.get("/v1/system/topology")
async def system_topology() -> dict[str, Any]:
    client = _get_http_client()
    try:
        r = await client.get("/topology")
        if r.status_code == 200:
            data = r.json()
            return {"status": "ok", "source": "rust-sidecar", **data}
    except Exception:
        pass
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
    except Exception:
        pass
    return {"peers": [], "count": 0}


@app.post("/v1/chat/completions")
async def chat_completions(payload: ChatRequest) -> Any:
    STATE.last_local_activity_ts = time.time()

    user_text = "\n".join(m.content for m in payload.messages if m.role == "user")
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
    control = RustControlPlaneClient(base_url=RUST_URL)
    tokens: list[str] = []
    async for tok in cooperative_generate(
        prompt=user_text,
        local_model_generate=_stub_local_generate,
        verify_draft=_stub_verify,
        control_plane=control,
        max_tokens=payload.max_tokens,
    ):
        tokens.append(tok)

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
    control = RustControlPlaneClient(base_url=RUST_URL)

    async for token in cooperative_generate(
        prompt=prompt,
        local_model_generate=_stub_local_generate,
        verify_draft=_stub_verify,
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
        await asyncio.sleep(0.01)  # Small delay for natural streaming feel

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
