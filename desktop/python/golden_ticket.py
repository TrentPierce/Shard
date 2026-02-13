"""Golden Ticket Security System for Sybil Attack Prevention

The Golden Ticket system provides cryptographic verification of Scout nodes
to prevent Sybil attacks in the Shard P2P network. Shards inject pre-solved
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
    
    # Generate a golden ticket (Shard side)
    gt = generator.maybe_inject_golden_ticket(normal_prompt)
    if gt["is_golden_ticket"]:
        # Store expected answer for later verification
        store_expected_answer(gt["request_id"], gt["expected_answer"])
    
    # Verify a response (Shard side)
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
import sqlite3
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

# File paths for persistence (JSON fallback)
BANNED_LIST_PATH = Path(os.getenv("SHARD_BANNED_LIST_PATH", "./data/banned_scouts.json"))
REPUTATION_DB_PATH = Path(os.getenv("SHARD_REPUTATION_DB_PATH", "./data/scout_reputation.json"))

# SQLite database path (preferred for production)
SQLITE_DB_PATH = Path(os.getenv("SHARD_SQLITE_DB_PATH", "./data/shard_reputation.db"))


# ─── SQLite Reputation Ledger ───────────────────────────────────────────────────

class SQLiteReputationLedger:
    """SQLite-backed reputation ledger for production deployments.
    
    This provides:
    - Persistent storage of scout reputations
    - Atomic updates for concurrent access
    - Better performance than JSON files for large datasets
    
    Schema:
        scout_reputation:
            - peer_id (TEXT PRIMARY KEY)
            - golden_attempts (INTEGER)
            - golden_correct (INTEGER)
            - first_seen (REAL)
            - last_seen (REAL)
            
        banned_scouts:
            - peer_id (TEXT PRIMARY KEY)
            - banned_at (REAL)
            - ban_duration_hours (INTEGER)
            - reason (TEXT)
            - failed_attempts (INTEGER)
    """
    
    def __init__(self, db_path: Path | None = None) -> None:
        self._db_path = db_path or SQLITE_DB_PATH
        self._conn: sqlite3.Connection | None = None
        self._lock = threading.RLock()
        
    def _ensure_connection(self) -> sqlite3.Connection:
        """Ensure database connection is established."""
        if self._conn is None:
            self._db_path.parent.mkdir(parents=True, exist_ok=True)
            self._conn = sqlite3.connect(str(self._db_path), check_same_thread=False)
            self._conn.row_factory = sqlite3.Row
            self._init_schema()
        return self._conn
    
    def _init_schema(self) -> None:
        """Initialize database schema."""
        conn = self._conn
        if conn is None:
            return
            
        conn.execute("""
            CREATE TABLE IF NOT EXISTS scout_reputation (
                peer_id TEXT PRIMARY KEY,
                golden_attempts INTEGER DEFAULT 0,
                golden_correct INTEGER DEFAULT 0,
                first_seen REAL NOT NULL,
                last_seen REAL NOT NULL
            )
        """)
        
        conn.execute("""
            CREATE TABLE IF NOT EXISTS banned_scouts (
                peer_id TEXT PRIMARY KEY,
                banned_at REAL NOT NULL,
                ban_duration_hours INTEGER NOT NULL,
                reason TEXT,
                failed_attempts INTEGER DEFAULT 0
            )
        """)
        
        conn.commit()
    
    def get_reputation(self, peer_id: str) -> ScoutReputation | None:
        """Get reputation for a scout peer."""
        with self._lock:
            conn = self._ensure_connection()
            row = conn.execute(
                "SELECT * FROM scout_reputation WHERE peer_id = ?",
                (peer_id,)
            ).fetchone()
            
            if row is None:
                return None
                
            return ScoutReputation(
                peer_id=row["peer_id"],
                golden_attempts=row["golden_attempts"],
                golden_correct=row["golden_correct"],
                first_seen=row["first_seen"],
                last_seen=row["last_seen"],
            )
    
    def save_reputation(self, rep: ScoutReputation) -> None:
        """Save or update reputation for a scout."""
        with self._lock:
            conn = self._ensure_connection()
            conn.execute("""
                INSERT OR REPLACE INTO scout_reputation 
                (peer_id, golden_attempts, golden_correct, first_seen, last_seen)
                VALUES (?, ?, ?, ?, ?)
            """, (
                rep.peer_id,
                rep.golden_attempts,
                rep.golden_correct,
                rep.first_seen,
                rep.last_seen,
            ))
            conn.commit()
    
    def is_banned(self, peer_id: str) -> bool:
        """Check if a scout is currently banned."""
        with self._lock:
            conn = self._ensure_connection()
            row = conn.execute(
                "SELECT banned_at, ban_duration_hours FROM banned_scouts WHERE peer_id = ?",
                (peer_id,)
            ).fetchone()
            
            if row is None:
                return False
                
            # Check if ban has expired
            elapsed_hours = (time.time() - row["banned_at"]) / 3600
            return elapsed_hours < row["ban_duration_hours"]
    
    def ban_scout(self, ban: BanEntry) -> None:
        """Ban a scout."""
        with self._lock:
            conn = self._ensure_connection()
            conn.execute("""
                INSERT OR REPLACE INTO banned_scouts
                (peer_id, banned_at, ban_duration_hours, reason, failed_attempts)
                VALUES (?, ?, ?, ?, ?)
            """, (
                ban.peer_id,
                ban.banned_at,
                ban.ban_duration_hours,
                ban.reason,
                ban.failed_attempts,
            ))
            conn.commit()
    
    def unban_scout(self, peer_id: str) -> bool:
        """Unban a scout. Returns True if scout was unbanned."""
        with self._lock:
            conn = self._ensure_connection()
            cursor = conn.execute(
                "DELETE FROM banned_scouts WHERE peer_id = ?",
                (peer_id,)
            )
            conn.commit()
            return cursor.rowcount > 0
    
    def get_all_reputations(self) -> dict[str, ScoutReputation]:
        """Get all scout reputations."""
        with self._lock:
            conn = self._ensure_connection()
            rows = conn.execute("SELECT * FROM scout_reputation").fetchall()
            return {
                row["peer_id"]: ScoutReputation(
                    peer_id=row["peer_id"],
                    golden_attempts=row["golden_attempts"],
                    golden_correct=row["golden_correct"],
                    first_seen=row["first_seen"],
                    last_seen=row["last_seen"],
                )
                for row in rows
            }
    
    def get_all_banned(self) -> dict[str, BanEntry]:
        """Get all active bans."""
        with self._lock:
            conn = self._ensure_connection()
            rows = conn.execute("SELECT * FROM banned_scouts").fetchall()
            result = {}
            for row in rows:
                ban = BanEntry(
                    peer_id=row["peer_id"],
                    banned_at=row["banned_at"],
                    ban_duration_hours=row["ban_duration_hours"],
                    reason=row["reason"],
                    failed_attempts=row["failed_attempts"],
                )
                if ban.is_active:
                    result[row["peer_id"]] = ban
            return result
    
    def close(self) -> None:
        """Close database connection."""
        with self._lock:
            if self._conn:
                self._conn.close()
                self._conn = None

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
    {"prompt": 'What letter comes after "b" in the alphabet?', "expected": "c", "type": "exact"},
    {"prompt": "Capitalize the word 'test'", "expected": "TEST", "type": "exact"},
    
    # Factual knowledge (common, verifiable facts)
    {"prompt": "What is the capital of France?", "expected": "Paris", "type": "contains"},
    {"prompt": "How many days are in a week?", "expected": "7", "type": "exact"},
    {"prompt": "What planet is known as the Red Planet?", "expected": "Mars", "type": "contains"},
    {"prompt": "How many continents are there on Earth?", "expected": "7", "type": "exact"},
    {"prompt": "What is the freezing point of water in Celsius?", "expected": "0", "type": "contains"},
    {"prompt": "How many sides does a triangle have?", "expected": "3", "type": "exact"},
    {"prompt": "What color is the sky on a clear day?", "expected": "blue", "type": "contains"},
    {"prompt": "How many hours are in a day?", "expected": "24", "type": "exact"},
    {"prompt": "What is the opposite of 'hot'?", "expected": "cold", "type": "contains"},
    {"prompt": "How many minutes are in an hour?", "expected": "60", "type": "exact"},
]


# ─── Data Classes ─────────────────────────────────────────────────────────────


@dataclass
class GoldenTicket:
    """A pre-solved prompt for verifying scout honesty."""
    
    request_id: str
    prompt: str
    expected_answer: str
    tolerance: Literal["exact", "contains", "numeric"]
    is_golden_ticket: bool = True
    created_at: float = field(default_factory=time.time)
    
    def to_dict(self) -> dict[str, object]:
        """Serialize to dictionary for JSON transmission."""
        return {
            "request_id": self.request_id,
            "prompt": self.prompt,
            "expected_answer": self.expected_answer,
            "tolerance": self.tolerance,
            "is_golden_ticket": self.is_golden_ticket,
            "created_at": self.created_at,
        }


@dataclass
class ScoutReputation:
    """Reputation tracking for a single scout peer."""
    
    peer_id: str
    golden_attempts: int = 0
    golden_correct: int = 0
    last_seen: float = field(default_factory=time.time)
    first_seen: float = field(default_factory=time.time)
    
    @property
    def accuracy(self) -> float:
        """Calculate accuracy rate (0.0 to 1.0)."""
        if self.golden_attempts == 0:
            return 1.0  # New scouts start with neutral reputation
        return self.golden_correct / self.golden_attempts
    
    def record_attempt(self, correct: bool) -> None:
        """Record a Golden Ticket attempt result."""
        self.golden_attempts += 1
        if correct:
            self.golden_correct += 1
        self.last_seen = time.time()
    
    def to_dict(self) -> dict[str, object]:
        """Serialize to dictionary for persistence."""
        return {
            "peer_id": self.peer_id,
            "golden_attempts": self.golden_attempts,
            "golden_correct": self.golden_correct,
            "last_seen": self.last_seen,
            "first_seen": self.first_seen,
            "accuracy": self.accuracy,
        }


@dataclass
class BanEntry:
    """Ban record for a misbehaving scout."""
    
    peer_id: str
    banned_at: float = field(default_factory=time.time)
    ban_duration_hours: int = DEFAULT_BAN_DURATION_HOURS
    reason: str = "Failed Golden Ticket verification"
    failed_attempts: int = 0
    
    @property
    def is_active(self) -> bool:
        """Check if the ban is still active."""
        elapsed_hours = (time.time() - self.banned_at) / 3600
        return elapsed_hours < self.ban_duration_hours
    
    @property
    def expires_at(self) -> float:
        """Get the ban expiration timestamp."""
        return self.banned_at + (self.ban_duration_hours * 3600)
    
    def to_dict(self) -> dict[str, object]:
        """Serialize to dictionary for persistence."""
        return {
            "peer_id": self.peer_id,
            "banned_at": self.banned_at,
            "ban_duration_hours": self.ban_duration_hours,
            "reason": self.reason,
            "failed_attempts": self.failed_attempts,
            "is_active": self.is_active,
            "expires_at": self.expires_at,
        }


# ─── Golden Ticket Generator ─────────────────────────────────────────────────


class GoldenTicketGenerator:
    """Generates and manages Golden Tickets for Sybil attack prevention.
    
    This class handles:
    - Random injection of Golden Tickets into the work stream
    - Tracking active Golden Tickets awaiting verification
    - Managing scout reputation scores
    - Banning scouts that fail verification
    
    Thread-safe for concurrent access from multiple async tasks.
    """
    
    def __init__(
        self,
        injection_rate: float = DEFAULT_INJECTION_RATE,
        reputation_threshold: float = DEFAULT_REPUTATION_THRESHOLD,
        min_attempts_before_ban: int = DEFAULT_MIN_ATTEMPTS_BEFORE_BAN,
        ban_duration_hours: int = DEFAULT_BAN_DURATION_HOURS,
    ) -> None:
        """Initialize the Golden Ticket generator.
        
        Args:
            injection_rate: Probability (0.0-1.0) of injecting a Golden Ticket
            reputation_threshold: Minimum accuracy required to avoid banning
            min_attempts_before_ban: Minimum Golden Tickets before ban decision
            ban_duration_hours: How long bans last (0 = permanent)
        """
        self.injection_rate = max(0.0, min(1.0, injection_rate))
        self.reputation_threshold = reputation_threshold
        self.min_attempts_before_ban = min_attempts_before_ban
        self.ban_duration_hours = ban_duration_hours
        
        # Thread-safe storage
        self._lock = threading.RLock()
        self._active_tickets: dict[str, GoldenTicket] = {}  # request_id -> ticket
        self._reputation: dict[str, ScoutReputation] = {}  # peer_id -> reputation
        self._banned: dict[str, BanEntry] = {}  # peer_id -> ban entry
        
        # Cryptographically secure random number generator
        self._secure_random = secrets.SystemRandom()
        
        # Load persisted data
        self._load_banned_list()
        self._load_reputation_db()
        
        LOGGER.info(
            "Golden Ticket system initialized (injection_rate=%.2f, threshold=%.2f)",
            self.injection_rate,
            self.reputation_threshold,
        )
    
    def maybe_inject_golden_ticket(
        self,
        normal_prompt: str,
        request_id: str | None = None,
    ) -> dict[str, object]:
        """Potentially inject a Golden Ticket into the request stream.
        
        Uses cryptographically secure randomness to decide whether to inject.
        The decision is unpredictable to prevent scouts from detecting patterns.
        
        Args:
            normal_prompt: The original user prompt (returned if no injection)
            request_id: Optional request ID (generated if not provided)
            
        Returns:
            Dictionary with ticket details. Check 'is_golden_ticket' field.
        """
        # Generate request ID if not provided
        if request_id is None:
            request_id = f"gt_{secrets.token_hex(16)}"
        
        # Random decision using secure RNG
        if self._secure_random.random() >= self.injection_rate:
            # No injection - return normal prompt
            return {
                "request_id": request_id,
                "is_golden_ticket": False,
                "prompt": normal_prompt,
                "expected_answer": None,
                "tolerance": None,
            }
        
        # Inject Golden Ticket
        ticket = self._create_golden_ticket(request_id)
        
        with self._lock:
            self._active_tickets[request_id] = ticket
        
        LOGGER.debug("Injected Golden Ticket: %s (prompt: %s)", request_id, ticket.prompt[:50])
        
        return ticket.to_dict()
    
    def _create_golden_ticket(self, request_id: str) -> GoldenTicket:
        """Create a new Golden Ticket from templates."""
        # Select random template using secure RNG
        template = self._secure_random.choice(GOLDEN_TICKET_TEMPLATES)
        
        return GoldenTicket(
            request_id=request_id,
            prompt=template["prompt"],
            expected_answer=template["expected"],
            tolerance=template["type"],  # type: ignore[arg-type]
        )
    
    def verify_response(
        self,
        request_id: str,
        scout_peer_id: str,
        scout_response: str,
    ) -> bool | None:
        """Verify a scout's response against an active Golden Ticket.
        
        Args:
            request_id: The ID of the work request
            scout_peer_id: The peer ID of the responding scout
            scout_response: The text response from the scout
            
        Returns:
            True if verified correctly, False if failed, None if not a Golden Ticket
        """
        with self._lock:
            ticket = self._active_tickets.get(request_id)
        
        if ticket is None:
            # Not a Golden Ticket - normal work
            return None
        
        # Verify the response
        is_correct = self._check_answer(
            scout_response,
            ticket.expected_answer,
            ticket.tolerance,
        )
        
        # Update reputation
        self._update_reputation(scout_peer_id, is_correct)
        
        # Clean up active ticket
        with self._lock:
            self._active_tickets.pop(request_id, None)
        
        LOGGER.info(
            "Golden Ticket %s verification: peer=%s correct=%s",
            request_id,
            scout_peer_id,
            is_correct,
        )
        
        return is_correct
    
    def _check_answer(
        self,
        response: str,
        expected: str,
        tolerance: Literal["exact", "contains", "numeric"],
    ) -> bool:
        """Check if a response matches the expected answer.
        
        Args:
            response: The scout's response text
            expected: The expected answer
            tolerance: Match type - exact, contains, or numeric
            
        Returns:
            True if the answer is correct
        """
        response_clean = response.strip().lower()
        expected_clean = expected.strip().lower()
        
        if tolerance == "exact":
            # Exact match (case-insensitive)
            return response_clean == expected_clean
        
        elif tolerance == "contains":
            # Expected answer appears somewhere in response
            return expected_clean in response_clean
        
        elif tolerance == "numeric":
            # Extract numeric values and compare
            response_nums = re.findall(r'-?\d+\.?\d*', response)
            expected_num = re.findall(r'-?\d+\.?\d*', expected)
            
            if expected_num:
                expected_val = float(expected_num[0])
                # Check if any extracted number matches
                for num_str in response_nums:
                    try:
                        if abs(float(num_str) - expected_val) < 0.01:
                            return True
                    except ValueError:
                        continue
            return response_clean == expected_clean
        
        return False
    
    def _update_reputation(self, peer_id: str, correct: bool) -> None:
        """Update a scout's reputation based on Golden Ticket result."""
        with self._lock:
            if peer_id not in self._reputation:
                self._reputation[peer_id] = ScoutReputation(peer_id=peer_id)
            
            self._reputation[peer_id].record_attempt(correct)
            
            # Check if scout should be banned
            if self._should_ban(peer_id):
                self._ban_scout(peer_id)
        
        # Persist changes
        self._save_reputation_db()
    
    def _should_ban(self, peer_id: str) -> bool:
        """Determine if a scout should be banned based on reputation."""
        reputation = self._reputation.get(peer_id)
        if reputation is None:
            return False
        
        # Need minimum attempts before ban decision
        if reputation.golden_attempts < self.min_attempts_before_ban:
            return False
        
        # Check if accuracy is below threshold
        return reputation.accuracy < self.reputation_threshold
    
    def _ban_scout(self, peer_id: str) -> None:
        """Ban a scout for failing Golden Tickets."""
        reputation = self._reputation.get(peer_id)
        failed = 0
        if reputation:
            failed = reputation.golden_attempts - reputation.golden_correct
        
        ban_entry = BanEntry(
            peer_id=peer_id,
            failed_attempts=failed,
            ban_duration_hours=self.ban_duration_hours,
        )
        
        self._banned[peer_id] = ban_entry
        self._save_banned_list()
        
        LOGGER.warning(
            "Scout BANNED: peer=%s failed_attempts=%d accuracy=%.2f",
            peer_id,
            failed,
            reputation.accuracy if reputation else 0.0,
        )
    
    def is_scout_banned(self, peer_id: str) -> bool:
        """Check if a scout is currently banned.
        
        Also cleans up expired bans automatically.
        """
        with self._lock:
            ban_entry = self._banned.get(peer_id)
            if ban_entry is None:
                return False
            
            # Check if ban has expired
            if not ban_entry.is_active:
                # Remove expired ban
                del self._banned[peer_id]
                self._save_banned_list()
                LOGGER.info("Ban expired for peer: %s", peer_id)
                return False
            
            return True
    
    def get_reputation(self, peer_id: str) -> dict[str, object]:
        """Get reputation information for a scout."""
        with self._lock:
            reputation = self._reputation.get(peer_id)
            if reputation is None:
                return {
                    "peer_id": peer_id,
                    "golden_attempts": 0,
                    "golden_correct": 0,
                    "accuracy": 1.0,
                    "status": "new",
                }
            
            is_banned = peer_id in self._banned and self._banned[peer_id].is_active
            
            return {
                "peer_id": peer_id,
                "golden_attempts": reputation.golden_attempts,
                "golden_correct": reputation.golden_correct,
                "accuracy": reputation.accuracy,
                "status": "banned" if is_banned else "active",
                "first_seen": reputation.first_seen,
                "last_seen": reputation.last_seen,
            }
    
    def get_all_reputations(self) -> dict[str, dict[str, object]]:
        """Get all scout reputations."""
        with self._lock:
            return {
                peer_id: rep.to_dict()
                for peer_id, rep in self._reputation.items()
            }
    
    def get_banned_list(self) -> dict[str, dict[str, object]]:
        """Get all currently banned scouts."""
        with self._lock:
            # Filter out expired bans
            active_bans = {
                peer_id: ban.to_dict()
                for peer_id, ban in self._banned.items()
                if ban.is_active
            }
            return active_bans
    
    def unban_scout(self, peer_id: str) -> bool:
        """Manually unban a scout (admin override).
        
        Returns:
            True if scout was unbanned, False if not found
        """
        with self._lock:
            if peer_id in self._banned:
                del self._banned[peer_id]
                self._save_banned_list()
                LOGGER.info("Scout manually unbanned: %s", peer_id)
                return True
            return False
    
    def reset_reputation(self, peer_id: str) -> bool:
        """Reset a scout's reputation (admin override).
        
        Returns:
            True if reputation was reset, False if not found
        """
        with self._lock:
            if peer_id in self._reputation:
                self._reputation[peer_id] = ScoutReputation(peer_id=peer_id)
                self._save_reputation_db()
                LOGGER.info("Scout reputation reset: %s", peer_id)
                return True
            return False
    
    def _load_banned_list(self) -> None:
        """Load banned list from persistent storage."""
        if not BANNED_LIST_PATH.exists():
            return
        
        try:
            with open(BANNED_LIST_PATH, "r") as f:
                data = json.load(f)
            
            with self._lock:
                for peer_id, ban_data in data.items():
                    self._banned[peer_id] = BanEntry(
                        peer_id=ban_data["peer_id"],
                        banned_at=ban_data["banned_at"],
                        ban_duration_hours=ban_data.get("ban_duration_hours", DEFAULT_BAN_DURATION_HOURS),
                        reason=ban_data.get("reason", "Failed Golden Ticket verification"),
                        failed_attempts=ban_data.get("failed_attempts", 0),
                    )
            
            LOGGER.info("Loaded %d banned scouts from %s", len(self._banned), BANNED_LIST_PATH)
        except Exception as e:
            LOGGER.error("Failed to load banned list: %s", e)
    
    def _save_banned_list(self) -> None:
        """Save banned list to persistent storage."""
        try:
            BANNED_LIST_PATH.parent.mkdir(parents=True, exist_ok=True)
            
            with self._lock:
                data = {
                    peer_id: ban.to_dict()
                    for peer_id, ban in self._banned.items()
                }
            
            with open(BANNED_LIST_PATH, "w") as f:
                json.dump(data, f, indent=2)
        except Exception as e:
            LOGGER.error("Failed to save banned list: %s", e)
    
    def _load_reputation_db(self) -> None:
        """Load reputation database from persistent storage."""
        if not REPUTATION_DB_PATH.exists():
            return
        
        try:
            with open(REPUTATION_DB_PATH, "r") as f:
                data = json.load(f)
            
            with self._lock:
                for peer_id, rep_data in data.items():
                    self._reputation[peer_id] = ScoutReputation(
                        peer_id=rep_data["peer_id"],
                        golden_attempts=rep_data.get("golden_attempts", 0),
                        golden_correct=rep_data.get("golden_correct", 0),
                        last_seen=rep_data.get("last_seen", time.time()),
                        first_seen=rep_data.get("first_seen", time.time()),
                    )
            
            LOGGER.info("Loaded %d scout reputations from %s", len(self._reputation), REPUTATION_DB_PATH)
        except Exception as e:
            LOGGER.error("Failed to load reputation database: %s", e)
    
    def _save_reputation_db(self) -> None:
        """Save reputation database to persistent storage."""
        try:
            REPUTATION_DB_PATH.parent.mkdir(parents=True, exist_ok=True)
            
            with self._lock:
                data = {
                    peer_id: rep.to_dict()
                    for peer_id, rep in self._reputation.items()
                }
            
            with open(REPUTATION_DB_PATH, "w") as f:
                json.dump(data, f, indent=2)
        except Exception as e:
            LOGGER.error("Failed to save reputation database: %s", e)


# ─── Module-Level Functions ──────────────────────────────────────────────────

# Global generator instance (singleton pattern)
_generator_instance: GoldenTicketGenerator | None = None
_generator_lock = threading.Lock()


def get_generator() -> GoldenTicketGenerator:
    """Get the global Golden Ticket generator instance.
    
    Creates the instance on first call using environment configuration.
    """
    global _generator_instance
    
    if _generator_instance is None:
        with _generator_lock:
            if _generator_instance is None:
                # Read configuration from environment
                injection_rate = float(os.getenv("SHARD_GOLDEN_TICKET_RATE", DEFAULT_INJECTION_RATE))
                threshold = float(os.getenv("SHARD_REPUTATION_THRESHOLD", DEFAULT_REPUTATION_THRESHOLD))
                min_attempts = int(os.getenv("SHARD_MIN_ATTEMPTS_BEFORE_BAN", DEFAULT_MIN_ATTEMPTS_BEFORE_BAN))
                ban_hours = int(os.getenv("SHARD_BAN_DURATION_HOURS", DEFAULT_BAN_DURATION_HOURS))
                
                _generator_instance = GoldenTicketGenerator(
                    injection_rate=injection_rate,
                    reputation_threshold=threshold,
                    min_attempts_before_ban=min_attempts,
                    ban_duration_hours=ban_hours,
                )
    
    return _generator_instance


def verify_golden_ticket(
    request_id: str,
    scout_peer_id: str,
    scout_response: str,
) -> bool | None:
    """Verify a scout's response against a Golden Ticket.
    
    Convenience function that uses the global generator instance.
    
    Args:
        request_id: The ID of the work request
        scout_peer_id: The peer ID of the responding scout
        scout_response: The text response from the scout
        
    Returns:
        True if verified correctly, False if failed, None if not a Golden Ticket
    """
    generator = get_generator()
    return generator.verify_response(request_id, scout_peer_id, scout_response)


def is_scout_banned(peer_id: str) -> bool:
    """Check if a scout is currently banned.
    
    Convenience function that uses the global generator instance.
    """
    generator = get_generator()
    return generator.is_scout_banned(peer_id)


def get_scout_reputation(peer_id: str) -> dict[str, object]:
    """Get reputation information for a scout.
    
    Convenience function that uses the global generator instance.
    """
    generator = get_generator()
    return generator.get_reputation(peer_id)


def maybe_inject_golden_ticket(
    normal_prompt: str,
    request_id: str | None = None,
) -> dict[str, object]:
    """Potentially inject a Golden Ticket into the request stream.
    
    Convenience function that uses the global generator instance.
    """
    generator = get_generator()
    return generator.maybe_inject_golden_ticket(normal_prompt, request_id)


def get_all_banned_scouts() -> dict[str, dict[str, object]]:
    """Get all currently banned scouts."""
    generator = get_generator()
    return generator.get_banned_list()


def unban_scout(peer_id: str) -> bool:
    """Manually unban a scout (admin override)."""
    generator = get_generator()
    return generator.unban_scout(peer_id)


def reset_scout_reputation(peer_id: str) -> bool:
    """Reset a scout's reputation (admin override)."""
    generator = get_generator()
    return generator.reset_reputation(peer_id)
