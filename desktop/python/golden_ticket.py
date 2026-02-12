"""Golden Ticket Security System for Sybil Attack Prevention

The Golden Ticket system provides cryptographic verification of Scout nodes
to prevent Sybil attacks in the Shard P2P network. Oracles inject pre-solved
prompts at random intervals (configurable, default 5%) to verify that Scouts
are honestly performing inference work.

Key Concepts:
- Golden Tickets: Pre-solved prompts with known answers injected into the work stream
- Reputation Tracking: Per-peer reputation scores based on Golden Ticket accuracy
- Banning: Scouts that fail Golden Tickets are banned from the network
- Cryptographic Randomness: Injection uses secrets.SystemRandom for unpredictability

Usage:
    from golden_ticket import (
        GoldenTicketGenerator,
        verify_golden_ticket,
        is_scout_banned,
        get_scout_reputation,
    )
    
    # Generate a golden ticket (Oracle side)
    gt = generator.maybe_inject_golden_ticket(normal_prompt)
    if gt["is_golden_ticket"]:
        # Store expected answer for later verification
        store_expected_answer(gt["request_id"], gt["expected_answer"])
    
    # Verify a response (Oracle side)
    is_valid = verify_golden_ticket(
        request_id=work_id,
        scout_response=scout_text,
        expected_answer=stored_answer,
        tolerance="exact",
    )
    
    # Check if scout is banned
    if is_scout_banned(scout_peer_id):
        reject_work_from(scout_peer_id)
"""

from __future__ import annotations

import json
import logging
import os
import re
import secrets
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

LOGGER = logging.getLogger("shard.golden_ticket")

# ─── Configuration ───────────────────────────────────────────────────────────

DEFAULT_INJECTION_RATE = 0.05  # 5% of requests
DEFAULT_REPUTATION_THRESHOLD = 0.7  # 70% accuracy required
DEFAULT_MIN_ATTEMPTS_BEFORE_BAN = 3  # Minimum golden tickets before ban decision
DEFAULT_BAN_DURATION_HOURS = 24  # Ban duration in hours

# File paths for persistence
BANNED_LIST_PATH = Path(os.getenv("SHARD_BANNED_LIST_PATH", "./data/banned_scouts.json"))
REPUTATION_DB_PATH = Path(os.getenv("SHARD_REPUTATION_DB_PATH", "./data/scout_reputation.json"))

# ─── Golden Ticket Templates ──────────────────────────────────────────────────

# Pre-defined prompts with objectively verifiable answers
# These are designed to be simple but require actual model inference
GOLDEN_TICKET_TEMPLATES: list[dict[str, str]] = [
    # Mathematical reasoning
    {"prompt": "What is 2+2?", "expected": "4", "type": "exact"},
    {"prompt": "Calculate 15 * 7", "expected": "105", "type": "exact"},
    {"prompt": "What is the square root of 144?", "expected": "12", "type": "exact"},
    {"prompt": "What is 100 divided by 4?", "expected": "25", "type": "exact"},
    {"prompt": "Calculate 17 + 28", "expected": "45", "type": "exact"},
    {"prompt": "What is 9 squared?", "expected": "81", "type": "exact"},
    {"prompt": "What is the sum of 123 and 456?", "expected": "579", "type": "exact"},
    {"prompt": "Calculate 50% of 200", "expected": "100", "type": "exact"},
    
    # String manipulation
    {"prompt": 'What is the third word in "The quick brown fox"?', "expected": "brown", "type": "exact"},
    {"prompt": "Spell 'hello' backwards", "expected": "olleh", "type": "exact"},
    {"prompt": "How many letters are in the word 'javascript'?", "expected": "10", "type": "exact"},
    {"prompt": 'What letter comes after 