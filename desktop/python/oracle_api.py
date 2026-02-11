"""OpenAI-compatible local Oracle API scaffold.

Service roles:
- Driver API in Python (OpenAI-compatible)
- Sidecar networking daemon in Rust (libp2p)
- In-process bitnet runtime via ctypes bridge
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any
import json
import os
import time

from fastapi import FastAPI
from pydantic import BaseModel, Field

from bitnet.ctypes_bridge import ShardEngineConfig, ShardEngineRuntime


app = FastAPI(title="Shard Oracle API", version="0.3.0")
TOPOLOGY_HINT_PATH = "/tmp/shard-topology.json"


@dataclass
class NodeState:
    last_local_activity_ts: float = time.time()
    idle_after_seconds: int = 30

    @property
    def is_idle(self) -> bool:
        return (time.time() - self.last_local_activity_ts) >= self.idle_after_seconds


STATE = NodeState()
BITNET: ShardEngineRuntime | None = None


def maybe_load_bitnet() -> ShardEngineRuntime | None:
    global BITNET
    if BITNET is not None:
        return BITNET

    lib_path = os.getenv("SHARD_ENGINE_LIB", os.getenv("BITNET_LIB", ""))
    model_path = os.getenv("SHARD_MODEL_PATH", os.getenv("BITNET_MODEL", ""))
    if not lib_path or not model_path:
        return None

    BITNET = ShardEngineRuntime(ShardEngineConfig(lib_path=lib_path, model_path=model_path))
    return BITNET


class Message(BaseModel):
    role: str
    content: str


class ChatRequest(BaseModel):
    model: str = Field(default="shard-hybrid")
    messages: list[Message]
    temperature: float = Field(default=0.7)
    max_tokens: int = Field(default=256)


class Choice(BaseModel):
    index: int
    message: dict[str, str]
    finish_reason: str = "stop"


class ChatResponse(BaseModel):
    id: str
    object: str = "chat.completion"
    choices: list[Choice]
    usage: dict[str, int]


@app.get("/health")
def health() -> dict[str, Any]:
    return {
        "status": "ok",
        "idle": STATE.is_idle,
        "accepting_swarm_jobs": STATE.is_idle,
        "rust_control_plane": "grpc+uds:///tmp/shard-control.sock",
        "bitnet_loaded": BITNET is not None,
    }


@app.get("/v1/system/topology")
def system_topology() -> dict[str, Any]:
    if os.path.exists(TOPOLOGY_HINT_PATH):
        with open(TOPOLOGY_HINT_PATH, "r", encoding="utf-8") as f:
            data = json.load(f)
        return {
            "status": "ok",
            "source": "rust-sidecar",
            **data,
        }
    return {
        "status": "degraded",
        "source": "fallback",
        "oracle_webrtc_multiaddr": None,
        "detail": "Rust topology hint not published yet",
    }


@app.post("/v1/chat/completions", response_model=ChatResponse)
def chat_completions(payload: ChatRequest) -> ChatResponse:
    STATE.last_local_activity_ts = time.time()

    user_text = "\n".join(m.content for m in payload.messages if m.role == "user")

    runtime = maybe_load_bitnet()
    accepted, corrected = [], []
    if runtime is not None:
        token_ids = [1, 2, 3, 4]
        runtime.eval_tokens(token_ids)
        top = runtime.get_top_k_tokens(k=3)
        accepted = [str(t) for t in top[:2]]
        corrected = [str(top[2])] if len(top) > 2 else []

    synthetic = (
        "[scaffold] Hybrid swarm response for prompt: "
        f"{user_text[:160]}"
        f" | verify(accepted={len(accepted)}, corrected={len(corrected)})"
    )

    return ChatResponse(
        id="chatcmpl-scaffold",
        choices=[Choice(index=0, message={"role": "assistant", "content": synthetic})],
        usage={"prompt_tokens": len(user_text.split()), "completion_tokens": 8, "total_tokens": 0},
    )
