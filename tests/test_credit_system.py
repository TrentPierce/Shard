from __future__ import annotations

import json
import time

import pytest

from credit_system import InvalidPoW, SybilDetector, WalletRateLimiter


def test_normal_earning_succeeds():
    limiter = WalletRateLimiter(max_hourly_shards=10)
    limiter.check_and_record("w1", 2.0, now=1000)
    limiter.check_and_record("w1", 3.0, now=1100)


def test_rate_limit_blocks_excess_earning():
    limiter = WalletRateLimiter(max_hourly_shards=5)
    limiter.check_and_record("w1", 4.0, now=1000)
    with pytest.raises(Exception):
        limiter.check_and_record("w1", 2.0, now=1001)


def test_rate_limit_window_slides_correctly():
    limiter = WalletRateLimiter(max_hourly_shards=5)
    limiter.check_and_record("w1", 5.0, now=1000)
    with pytest.raises(Exception):
        limiter.check_and_record("w1", 1.0, now=1200)
    limiter.check_and_record("w1", 1.0, now=5001)


def test_low_acceptance_rate_blocks_credits():
    limiter = WalletRateLimiter(min_acceptance_rate=0.4)
    for _ in range(30):
        limiter.record_submission("w1", True)
    for _ in range(70):
        limiter.record_submission("w1", False)
    assert limiter.is_quality_eligible("w1") is False


def test_acceptance_rate_exactly_at_threshold_allowed():
    limiter = WalletRateLimiter(min_acceptance_rate=0.4)
    for _ in range(40):
        limiter.record_submission("w1", True)
    for _ in range(60):
        limiter.record_submission("w1", False)
    assert limiter.acceptance_rate("w1") == 0.4
    assert limiter.is_quality_eligible("w1") is True


def test_sybil_flag_on_subnet_overflow(tmp_path):
    detector = SybilDetector(max_nodes_per_subnet_per_hour=2, audit_log_path=str(tmp_path / "audit.jsonl"))
    detector.record_new_node("a", "10.0.1.1", 1000)
    detector.record_new_node("b", "10.0.1.2", 1001)
    detector.record_new_node("c", "10.0.1.3", 1002)
    assert detector.is_flagged("c")


def test_sybil_no_flag_below_threshold(tmp_path):
    detector = SybilDetector(max_nodes_per_subnet_per_hour=3, audit_log_path=str(tmp_path / "audit.jsonl"))
    detector.record_new_node("a", "10.0.1.1", 1000)
    detector.record_new_node("b", "10.0.1.2", 1001)
    assert detector.is_flagged("a") is False
    assert detector.is_flagged("b") is False


def test_pow_required_before_earning(tmp_path):
    detector = SybilDetector(min_pow_difficulty=8, audit_log_path=str(tmp_path / "audit.jsonl"))
    assert detector.is_pow_verified("w1") is False


def test_invalid_pow_rejected(tmp_path):
    detector = SybilDetector(min_pow_difficulty=8, audit_log_path=str(tmp_path / "audit.jsonl"))
    with pytest.raises(InvalidPoW):
        detector.record_pow_solution("w1", {"hash_hex": "ff" * 32})


def test_audit_log_written_on_sybil_flag(tmp_path):
    path = tmp_path / "audit.jsonl"
    detector = SybilDetector(max_nodes_per_subnet_per_hour=1, audit_log_path=str(path))
    detector.record_new_node("a", "10.1.1.1", time.time())
    detector.record_new_node("b", "10.1.1.2", time.time())
    assert path.exists()
    lines = path.read_text(encoding="utf-8").strip().splitlines()
    payload = json.loads(lines[-1])
    assert payload["event"] == "sybil_flag"
    assert payload["wallet"] == "b"
