"""Standalone inference service used by Dockerfile.inference.

This service is intentionally minimal:
- Exposes health and generate endpoints.
- Loads BitNet runtime when BITNET_LIB and BITNET_MODEL are configured.
- Returns a clear 503 when runtime is not configured.
"""

from __future__ import annotations

import asyncio
import os
from typing import Any

from fastapi import FastAPI, HTTPException, status
from pydantic import BaseModel, Field

from bitnet.ctypes_bridge import BitNetConfig, BitNetRuntime

app = FastAPI(title="Shard Inference Service", version="0.4.4")

_runtime: BitNetRuntime | None = None
_runtime_lock = asyncio.Lock()


class GenerateRequest(BaseModel):
    prompt: str = Field(..., min_length=1, max_length=16000)
    max_tokens: int = Field(default=128, ge=1, le=2048)


async def _get_runtime() -> BitNetRuntime | None:
    global _runtime
    if _runtime is not None:
        return _runtime

    lib_path = os.getenv("BITNET_LIB", "").strip()
    model_path = os.getenv("BITNET_MODEL", "").strip()
    if not lib_path or not model_path:
        return None

    async with _runtime_lock:
        if _runtime is not None:
            return _runtime
        _runtime = await asyncio.to_thread(
            BitNetRuntime,
            BitNetConfig(lib_path=lib_path, model_path=model_path),
        )
    return _runtime


@app.get("/health")
async def health() -> dict[str, Any]:
    runtime = await _get_runtime()
    return {
        "status": "ok",
        "bitnet_loaded": runtime is not None,
    }


@app.post("/generate")
async def generate(payload: GenerateRequest) -> dict[str, Any]:
    runtime = await _get_runtime()
    if runtime is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="BitNet runtime unavailable. Configure BITNET_LIB and BITNET_MODEL.",
        )

    try:
        await asyncio.to_thread(runtime.rollback, 999999)
        await asyncio.to_thread(runtime.eval_text, payload.prompt)
        tokens: list[str] = []
        for _ in range(payload.max_tokens):
            token = await asyncio.to_thread(runtime.generate_next_token, tokens)
            if token is None:
                break
            tokens.append(token)
            await asyncio.to_thread(runtime.eval_text, token)
    except Exception as exc:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail=f"inference failed: {exc}",
        ) from exc

    return {
        "text": " ".join(tokens),
        "tokens": tokens,
        "count": len(tokens),
    }
