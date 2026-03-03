import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from integrations.overflow_router import SLAEnforcer


def test_sla_enforcer_healthy_below_threshold() -> None:
    enforcer = SLAEnforcer(p95_threshold_ms=3000.0, window_size=20, cooldown_seconds=1.0)
    for _ in range(20):
        enforcer.record_latency(500.0)
    assert enforcer.is_sla_healthy()
    assert enforcer.p95() == 500.0
    assert enforcer.breach_count() == 0


def test_sla_enforcer_breach_enters_cooldown_and_recovers() -> None:
    enforcer = SLAEnforcer(p95_threshold_ms=1000.0, window_size=10, cooldown_seconds=0.2)
    for _ in range(10):
        enforcer.record_latency(1500.0)
    assert not enforcer.is_sla_healthy()
    assert enforcer.breach_count() == 1

    time.sleep(0.25)
    assert enforcer.is_sla_healthy()


def test_sla_enforcer_counts_new_breach_after_cooldown() -> None:
    enforcer = SLAEnforcer(p95_threshold_ms=1000.0, window_size=5, cooldown_seconds=0.2)
    for _ in range(5):
        enforcer.record_latency(2000.0)
    assert enforcer.breach_count() == 1
    time.sleep(0.25)
    for _ in range(5):
        enforcer.record_latency(2200.0)
    assert enforcer.breach_count() == 2
