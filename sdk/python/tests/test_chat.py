from __future__ import annotations

import json

import httpx
import pytest

from shard.errors import InvalidRequestError


def test_chat_create_success(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/v1/chat/completions"
        return httpx.Response(200, json={"id": "c1", "choices": [{"index": 0, "text": "hi"}]})

    client = make_client(handler)
    resp = client.chat.completions.create(model="shard-hybrid", messages=[{"role": "user", "content": "hi"}])
    assert resp.id == "c1"
    assert resp.choices[0].text == "hi"


def test_chat_create_error(make_client):
    def handler(_: httpx.Request) -> httpx.Response:
        return httpx.Response(400, text="bad")

    client = make_client(handler)
    with pytest.raises(InvalidRequestError):
        client.chat.completions.create(model="x", messages=[])


def test_chat_stream_edge_empty_done(make_client):
    lines = [
        "data: " + json.dumps({"id": "chunk1", "choices": [{"delta": {"content": "a"}}]}),
        "data: [DONE]",
    ]

    def handler(_: httpx.Request) -> httpx.Response:
        return httpx.Response(200, content=("\n".join(lines)).encode("utf-8"))

    client = make_client(handler)
    chunks = list(client.chat.completions.create(model="x", messages=[{"role": "user", "content": "h"}], stream=True))
    assert len(chunks) == 1
    assert chunks[0].id == "chunk1"
