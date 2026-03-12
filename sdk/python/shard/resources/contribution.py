from __future__ import annotations

import hashlib
import json
import time
from typing import TYPE_CHECKING, Any

from nacl.signing import SigningKey

from shard.models.contribution import ContributionAck

if TYPE_CHECKING:
    from shard.client import Client


def _now_ms() -> int:
    return int(time.time() * 1000)


class ContributorSession:
    def __init__(
        self,
        client: "Client",
        signing_key: SigningKey,
        nonce_seed: int | None = None,
    ) -> None:
        self._client = client
        self._signing_key = signing_key
        self._nonce = nonce_seed if nonce_seed is not None else (_now_ms() * 1000)

    @property
    def public_key_hex(self) -> str:
        return self._signing_key.verify_key.encode().hex()

    @property
    def seed_hex(self) -> str:
        return self._signing_key.encode().hex()

    def _next_nonce(self) -> int:
        nonce = self._nonce
        self._nonce += 1
        return nonce

    @staticmethod
    def _payload_hash(payload: dict[str, Any]) -> str:
        body = json.dumps(payload, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        return hashlib.sha256(body).hexdigest()

    def _sign_payload(
        self,
        payload: dict[str, Any],
        *,
        nonce: int | None = None,
        timestamp_ms: int | None = None,
    ) -> dict[str, Any]:
        effective_nonce = nonce if nonce is not None else self._next_nonce()
        effective_timestamp = timestamp_ms if timestamp_ms is not None else _now_ms()
        payload_hash_hex = self._payload_hash(payload)
        signing_body = (
            f"{self.public_key_hex}|{effective_nonce}|{effective_timestamp}|{payload_hash_hex}"
        ).encode("utf-8")
        signature_hex = self._signing_key.sign(signing_body).signature.hex()
        return {
            "envelope": {
                "signer_pubkey_hex": self.public_key_hex,
                "nonce": effective_nonce,
                "timestamp_ms": effective_timestamp,
                "payload": payload,
                "payload_hash_hex": payload_hash_hex,
                "signature_hex": signature_hex,
            }
        }

    def _post_signed(self, path: str, payload: dict[str, Any]) -> ContributionAck:
        response = self._client._request("POST", path, json=self._sign_payload(payload))
        return ContributionAck.model_validate(response.json())

    def register_node(
        self,
        *,
        role: str = "verifier",
        capacity: int | None = None,
        timestamp_ms: int | None = None,
    ) -> ContributionAck:
        payload = {
            "node_pubkey": self.public_key_hex,
            "role": role,
            "capacity": capacity,
            "timestamp_ms": timestamp_ms if timestamp_ms is not None else _now_ms(),
        }
        return self._post_signed("/signed/register-node", payload)

    def heartbeat(
        self,
        *,
        role: str = "verifier",
        queue_depth: int = 0,
        node_latency_ms: int = 0,
        uptime_seconds: int = 0,
        capability_tier: str | None = None,
        gpu_available: bool | None = None,
        accepts_scout_work: bool | None = None,
        public_api: bool | None = None,
        public_api_addr: str | None = None,
        timestamp_ms: int | None = None,
    ) -> ContributionAck:
        payload = {
            "node_pubkey": self.public_key_hex,
            "role": role,
            "queue_depth": queue_depth,
            "node_latency_ms": node_latency_ms,
            "uptime_seconds": uptime_seconds,
            "capability_tier": capability_tier,
            "gpu_available": gpu_available,
            "accepts_scout_work": accepts_scout_work,
            "public_api": public_api,
            "public_api_addr": public_api_addr,
            "timestamp_ms": timestamp_ms if timestamp_ms is not None else _now_ms(),
        }
        return self._post_signed("/signed/heartbeat", payload)

    def report_metrics(
        self,
        *,
        role: str = "verifier",
        queue_depth: int,
        node_latency_ms: int,
        uptime_seconds: int,
        capability_tier: str | None = None,
        gpu_available: bool | None = None,
        accepts_scout_work: bool | None = None,
        public_api: bool | None = None,
        timestamp_ms: int | None = None,
    ) -> ContributionAck:
        payload = {
            "node_pubkey": self.public_key_hex,
            "role": role,
            "queue_depth": queue_depth,
            "node_latency_ms": node_latency_ms,
            "uptime_seconds": uptime_seconds,
            "capability_tier": capability_tier,
            "gpu_available": gpu_available,
            "accepts_scout_work": accepts_scout_work,
            "public_api": public_api,
            "timestamp_ms": timestamp_ms if timestamp_ms is not None else _now_ms(),
        }
        return self._post_signed("/signed/metrics-report", payload)

    def deregister_node(
        self,
        *,
        role: str = "verifier",
        capacity: int | None = None,
        timestamp_ms: int | None = None,
    ) -> ContributionAck:
        payload = {
            "node_pubkey": self.public_key_hex,
            "role": role,
            "capacity": capacity,
            "timestamp_ms": timestamp_ms if timestamp_ms is not None else _now_ms(),
        }
        return self._post_signed("/signed/deregister-node", payload)

    def set_participation(self, enabled: bool) -> dict[str, Any]:
        return self._client.node.set_participation(enabled)

    def status(self):
        return self._client.node.status()


class ContributionResource:
    def __init__(self, client: "Client") -> None:
        self._client = client

    def create_session(
        self,
        *,
        seed_hex: str | None = None,
        nonce_seed: int | None = None,
    ) -> ContributorSession:
        if seed_hex is None:
            signing_key = SigningKey.generate()
        else:
            raw = bytes.fromhex(seed_hex)
            if len(raw) != 32:
                raise ValueError("seed_hex must decode to 32 bytes")
            signing_key = SigningKey(raw)
        return ContributorSession(self._client, signing_key, nonce_seed=nonce_seed)

