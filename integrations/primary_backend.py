from __future__ import annotations

import os
from typing import Any

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, StreamingResponse


def _gpu_util() -> float:
    try:
        return max(0.0, min(1.0, float(os.getenv("GPU_UTILIZATION_RATIO", "0.9"))))
    except ValueError:
        return 0.9


BACKEND_NAME = os.getenv("BACKEND_NAME", "primary")
app = FastAPI(title=f"Shard {BACKEND_NAME} Backend")


@app.get("/health")
async def health() -> JSONResponse:
    return JSONResponse({"status": "ok", "backend": BACKEND_NAME})


@app.get("/metrics")
async def metrics() -> StreamingResponse:
    body = (
        "# HELP gpu_utilization_ratio Simulated GPU utilization ratio\n"
        "# TYPE gpu_utilization_ratio gauge\n"
        f"gpu_utilization_ratio {_gpu_util():.6f}\n"
    )
    return StreamingResponse(iter([body.encode("utf-8")]), media_type="text/plain; version=0.0.4")


@app.post("/v1/chat/completions")
async def chat_completions(request: Request):
    payload: dict[str, Any] = await request.json()
    stream = bool(payload.get("stream", False))
    prompt = ""
    messages = payload.get("messages") or []
    if messages and isinstance(messages, list):
        prompt = str(messages[-1].get("content", ""))[:128]

    if stream:
        chunk = (
            'data: {"id":"mock-stream","choices":[{"index":0,"delta":{"content":"ok"}}]}\n\n'
            "data: [DONE]\n\n"
        )
        return StreamingResponse(iter([chunk.encode("utf-8")]), media_type="text/event-stream")

    return JSONResponse(
        {
            "id": f"{BACKEND_NAME}-resp-1",
            "object": "chat.completion",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": f"{BACKEND_NAME}: {prompt}"}}],
            "usage": {"prompt_tokens": 8, "completion_tokens": 8, "total_tokens": 16},
        }
    )
