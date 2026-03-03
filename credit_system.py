from __future__ import annotations

import json
import time
from collections import defaultdict, deque
from dataclasses import dataclass
from pathlib import Path


@dataclass
class RateLimitExceeded(Exception):
    wallet: str


class WalletRateLimiter:
    def __init__(self, max_hourly_shards: float = 25.0, min_acceptance_rate: float = 0.4):
        self.max_hourly_shards = max_hourly_shards
        self.min_acceptance_rate = min_acceptance_rate
        self.events: dict[str, deque[tuple[float, float]]] = defaultdict(deque)
        self.acceptance: dict[str, deque[bool]] = defaultdict(lambda: deque(maxlen=100))

    def check_and_record(self, wallet: str, amount: float, now: float | None = None) -> None:
        ts = time.time() if now is None else now
        bucket = self.events[wallet]
        while bucket and ts - bucket[0][0] > 3600:
            bucket.popleft()
        total = sum(v for _, v in bucket)
        if total + amount > self.max_hourly_shards:
            raise RateLimitExceeded(wallet)
        bucket.append((ts, amount))

    def record_submission(self, wallet: str, accepted: bool) -> None:
        self.acceptance[wallet].append(accepted)

    def acceptance_rate(self, wallet: str) -> float:
        samples = self.acceptance[wallet]
        if not samples:
            return 0.0
        return sum(samples) / len(samples)

    def is_quality_eligible(self, wallet: str) -> bool:
        return self.acceptance_rate(wallet) >= self.min_acceptance_rate


@dataclass
class InvalidPoW(Exception):
    wallet: str


class SybilDetector:
    def __init__(
        self,
        max_nodes_per_subnet_per_hour: int = 5,
        min_pow_difficulty: int = 8,
        audit_log_path: str = "shard_audit.jsonl",
    ):
        self.max_nodes_per_subnet_per_hour = max_nodes_per_subnet_per_hour
        self.min_pow_difficulty = min_pow_difficulty
        self.audit_log_path = Path(audit_log_path)
        self.registrations: dict[str, deque[tuple[float, str]]] = defaultdict(deque)
        self.flagged: set[str] = set()
        self.pow_verified: set[str] = set()

    @staticmethod
    def _subnet(ip: str) -> str:
        return ".".join(ip.split(".")[:3])

    @staticmethod
    def _leading_zero_bits(hex_hash: str) -> int:
        data = bytes.fromhex(hex_hash)
        bits = 0
        for byte in data:
            if byte == 0:
                bits += 8
                continue
            bits += 8 - byte.bit_length()
            break
        return bits

    def record_new_node(self, wallet: str, ip: str, timestamp: float) -> None:
        subnet = self._subnet(ip)
        bucket = self.registrations[subnet]
        while bucket and timestamp - bucket[0][0] > 3600:
            bucket.popleft()
        bucket.append((timestamp, wallet))
        if len(bucket) > self.max_nodes_per_subnet_per_hour:
            self.flagged.add(wallet)
            self.audit_log_path.parent.mkdir(parents=True, exist_ok=True)
            with self.audit_log_path.open("a", encoding="utf-8") as f:
                json.dump(
                    {
                        "timestamp": timestamp,
                        "event": "sybil_flag",
                        "wallet": wallet,
                        "ip_subnet": subnet,
                        "subnet_node_count": len(bucket),
                        "reason": "subnet_node_limit_exceeded",
                    },
                    f,
                )
                f.write("\n")

    def is_flagged(self, wallet: str) -> bool:
        return wallet in self.flagged

    def record_pow_solution(self, wallet: str, solution: dict) -> None:
        try:
            bits = self._leading_zero_bits(solution["hash_hex"])
        except Exception as exc:
            raise InvalidPoW(wallet) from exc
        if bits < self.min_pow_difficulty:
            raise InvalidPoW(wallet)
        self.pow_verified.add(wallet)

    def is_pow_verified(self, wallet: str) -> bool:
        return wallet in self.pow_verified
