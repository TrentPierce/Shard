from __future__ import annotations

import math
import random
import time
from collections import defaultdict, deque
from dataclasses import dataclass
from typing import Protocol

ACCEPTANCE_THRESHOLD = 0.12
GOLDEN_TICKET_PROBABILITY = 0.05
GOLDEN_TICKET_MIN_SCORE_PCT = 60.0
MIN_ACCEPTANCE_RATE_PCT = 50.0
SUSPENSION_SECONDS = 24 * 60 * 60


class Verifier(Protocol):
    def logits_for_prompt(self, prompt: str, token_count: int) -> list[list[float]]:
        ...

    def generate_suffix(self, prompt: str, start_index: int, total_tokens: int) -> list[int]:
        ...

    def generate_full(self, prompt: str, total_tokens: int) -> list[int]:
        ...


@dataclass
class VerificationOutcome:
    accepted_prefix_tokens: list[int]
    verifier_suffix_tokens: list[int]
    accepted_count: int
    rejected_from: int | None
    kl_by_position: list[float]


@dataclass
class AuditLogEntry:
    wallet_id: str
    timestamp: float
    score_pct: float
    flagged: bool


class VerificationEngine:
    def __init__(
        self,
        acceptance_threshold: float = ACCEPTANCE_THRESHOLD,
        audit_probability: float = GOLDEN_TICKET_PROBABILITY,
        audit_min_score_pct: float = GOLDEN_TICKET_MIN_SCORE_PCT,
        min_acceptance_rate_pct: float = MIN_ACCEPTANCE_RATE_PCT,
    ) -> None:
        self.acceptance_threshold = acceptance_threshold
        self.audit_probability = audit_probability
        self.audit_min_score_pct = audit_min_score_pct
        self.min_acceptance_rate_pct = min_acceptance_rate_pct
        self.audit_log: list[AuditLogEntry] = []
        self.wallet_flagged_until: dict[str, float] = {}
        self.wallet_acceptance_samples: dict[str, deque[bool]] = defaultdict(lambda: deque(maxlen=100))

    @staticmethod
    def _softmax(logits: list[float]) -> list[float]:
        max_logit = max(logits)
        exps = [math.exp(value - max_logit) for value in logits]
        total = sum(exps)
        return [value / total for value in exps]

    def kl_divergence(self, draft_logits: list[float], verifier_logits: list[float]) -> float:
        p = self._softmax(draft_logits)
        q = self._softmax(verifier_logits)
        eps = 1e-12
        return sum(pi * math.log((pi + eps) / (qi + eps)) for pi, qi in zip(p, q))

    def verify_draft(
        self,
        prompt: str,
        draft_logits: list[list[float]],
        draft_tokens: list[int],
        verifier: Verifier,
    ) -> VerificationOutcome:
        verifier_logits = verifier.logits_for_prompt(prompt, len(draft_logits))
        accepted_tokens: list[int] = []
        kl_scores: list[float] = []
        first_reject: int | None = None

        for index, (draft_step, verifier_step) in enumerate(zip(draft_logits, verifier_logits)):
            kl = self.kl_divergence(draft_step, verifier_step)
            kl_scores.append(kl)
            if kl < self.acceptance_threshold:
                accepted_tokens.append(draft_tokens[index])
            else:
                first_reject = index
                break

        if first_reject is None:
            suffix: list[int] = []
        else:
            suffix = verifier.generate_suffix(prompt, first_reject, len(draft_logits) - first_reject)

        return VerificationOutcome(
            accepted_prefix_tokens=accepted_tokens,
            verifier_suffix_tokens=suffix,
            accepted_count=len(accepted_tokens),
            rejected_from=first_reject,
            kl_by_position=kl_scores,
        )

    def _record_acceptance_sample(self, wallet_id: str, accepted: bool) -> None:
        samples = self.wallet_acceptance_samples[wallet_id]
        samples.append(accepted)
        if len(samples) < samples.maxlen:
            return
        acceptance_pct = (sum(samples) / len(samples)) * 100.0
        if acceptance_pct < self.min_acceptance_rate_pct:
            self.wallet_flagged_until[wallet_id] = time.time() + SUSPENSION_SECONDS

    def is_wallet_flagged(self, wallet_id: str) -> bool:
        until = self.wallet_flagged_until.get(wallet_id)
        if until is None:
            return False
        return until > time.time()

    def maybe_run_golden_ticket_audit(
        self,
        wallet_id: str,
        prompt: str,
        draft_tokens: list[int],
        verifier: Verifier,
    ) -> bool:
        if random.random() >= self.audit_probability:
            return False

        reference = verifier.generate_full(prompt, len(draft_tokens))
        matches = 0
        for left, right in zip(draft_tokens, reference):
            if left == right:
                matches += 1
        score_pct = (matches / len(draft_tokens) * 100.0) if draft_tokens else 0.0
        flagged = score_pct < self.audit_min_score_pct
        if flagged:
            self.wallet_flagged_until[wallet_id] = time.time() + SUSPENSION_SECONDS

        self.audit_log.append(
            AuditLogEntry(
                wallet_id=wallet_id,
                timestamp=time.time(),
                score_pct=score_pct,
                flagged=flagged,
            )
        )
        return True

    def submit_draft(
        self,
        wallet_id: str,
        prompt: str,
        draft_logits: list[list[float]],
        draft_tokens: list[int],
        verifier: Verifier,
    ) -> VerificationOutcome:
        outcome = self.verify_draft(prompt, draft_logits, draft_tokens, verifier)
        accepted = outcome.accepted_count == len(draft_tokens)
        self._record_acceptance_sample(wallet_id, accepted)
        self.maybe_run_golden_ticket_audit(wallet_id, prompt, draft_tokens, verifier)
        return outcome
