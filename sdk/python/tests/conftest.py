from __future__ import annotations

import httpx
import pytest

from shard.client import Client


@pytest.fixture
def make_client():
    def _factory(handler):
        client = Client(base_url="http://testserver")
        client._http.close()
        transport = httpx.MockTransport(handler)
        client._http = httpx.Client(base_url="http://testserver", transport=transport)
        return client

    return _factory
