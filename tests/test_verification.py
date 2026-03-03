from __future__ import annotations

from unittest.mock import Mock, patch

from verification import VerificationEngine


def _peaked_logits(index: int, size: int = 8, peak: float = 8.0, other: float = -2.0) -> list[float]:
    row = [other] * size
    row[index] = peak
    return row


def test_high_quality_draft_accepted():
    verifier = Mock()
    verifier.logits_for_prompt.return_value = [_peaked_logits(1), _peaked_logits(2), _peaked_logits(3)]
    verifier.generate_suffix.return_value = []

    engine = VerificationEngine(acceptance_threshold=0.2)
    draft_logits = [_peaked_logits(1), _peaked_logits(2), _peaked_logits(3)]
    draft_tokens = [11, 12, 13]

    outcome = engine.verify_draft("hello", draft_logits, draft_tokens, verifier)

    assert outcome.accepted_count == 3
    assert outcome.rejected_from is None
    assert outcome.verifier_suffix_tokens == []


def test_low_quality_draft_rejected():
    verifier = Mock()
    verifier.logits_for_prompt.return_value = [_peaked_logits(0), _peaked_logits(0), _peaked_logits(0)]
    verifier.generate_suffix.return_value = [90, 91, 92]

    engine = VerificationEngine(acceptance_threshold=0.02)
    draft_logits = [[0.0] * 8, [0.0] * 8, [0.0] * 8]
    draft_tokens = [1, 2, 3]

    outcome = engine.verify_draft("hello", draft_logits, draft_tokens, verifier)

    assert outcome.accepted_count == 0
    assert outcome.rejected_from == 0
    assert outcome.verifier_suffix_tokens == [90, 91, 92]


def test_partial_acceptance():
    verifier = Mock()
    verifier.logits_for_prompt.return_value = [
        _peaked_logits(1),
        _peaked_logits(2),
        _peaked_logits(3),
        _peaked_logits(4),
        _peaked_logits(4),
    ]
    verifier.generate_suffix.return_value = [44, 45]

    draft_logits = [
        _peaked_logits(1),
        _peaked_logits(2),
        _peaked_logits(3),
        _peaked_logits(0),
        _peaked_logits(0),
    ]
    engine = VerificationEngine(acceptance_threshold=0.2)

    outcome = engine.verify_draft("prompt", draft_logits, [10, 11, 12, 13, 14], verifier)

    assert outcome.accepted_count == 3
    assert outcome.rejected_from == 3
    assert outcome.verifier_suffix_tokens == [44, 45]


def test_adversarial_plausible_wrong():
    verifier = Mock()
    verifier.logits_for_prompt.return_value = [_peaked_logits(1)]
    verifier.generate_suffix.return_value = [99]

    # High confidence on wrong token index 0.
    draft_logits = [_peaked_logits(0)]
    engine = VerificationEngine(acceptance_threshold=0.02)

    outcome = engine.verify_draft("prompt", draft_logits, [42], verifier)

    assert outcome.accepted_count == 0
    assert outcome.rejected_from == 0


def test_golden_ticket_audit_triggered():
    verifier = Mock()
    verifier.generate_full.return_value = [1, 2, 3]

    engine = VerificationEngine(audit_probability=0.1)

    with patch("verification.random.random", return_value=0.01):
        triggered = engine.maybe_run_golden_ticket_audit("wallet-a", "p", [1, 2, 4], verifier)

    assert triggered is True
    verifier.generate_full.assert_called_once()
    assert len(engine.audit_log) == 1
    assert engine.audit_log[0].wallet_id == "wallet-a"


def test_golden_ticket_wallet_flagging():
    verifier = Mock()
    verifier.generate_full.return_value = [7, 7, 7, 7]
    engine = VerificationEngine(audit_probability=1.0, audit_min_score_pct=80.0)

    with patch("verification.random.random", return_value=0.0):
        for _ in range(3):
            engine.maybe_run_golden_ticket_audit("wallet-b", "prompt", [1, 1, 1, 1], verifier)

    assert engine.is_wallet_flagged("wallet-b") is True
    assert any(entry.flagged for entry in engine.audit_log)


def test_acceptance_rate_below_threshold_triggers_flag():
    verifier = Mock()
    verifier.logits_for_prompt.return_value = [_peaked_logits(1)]
    verifier.generate_suffix.return_value = [7]

    engine = VerificationEngine(min_acceptance_rate_pct=50.0, audit_probability=0.0)

    for i in range(70):
        # force rejections
        engine.submit_draft(
            "wallet-c",
            "prompt",
            [_peaked_logits(0)],
            [1],
            verifier,
        )

    for i in range(30):
        # accepted
        engine.submit_draft(
            "wallet-c",
            "prompt",
            [_peaked_logits(1)],
            [1],
            verifier,
        )

    assert engine.is_wallet_flagged("wallet-c") is True
