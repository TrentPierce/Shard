from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class ChatMessage(BaseModel):
    model_config = ConfigDict(extra="ignore")
    role: str
    content: str


class ChatRequest(BaseModel):
    model_config = ConfigDict(extra="ignore")
    model: str
    messages: list[ChatMessage]
    stream: bool = False
    max_tokens: int | None = None


class ChatChoice(BaseModel):
    model_config = ConfigDict(extra="ignore")
    index: int = 0
    message: ChatMessage | None = None
    text: str | None = None
    finish_reason: str | None = None


class ChatResponse(BaseModel):
    model_config = ConfigDict(extra="ignore")
    id: str | None = None
    object: str | None = None
    created: int | None = None
    model: str | None = None
    choices: list[ChatChoice] = []


class ChatStreamChunk(BaseModel):
    model_config = ConfigDict(extra="ignore")
    id: str | None = None
    object: str | None = None
    created: int | None = None
    model: str | None = None
    choices: list[dict] = []
