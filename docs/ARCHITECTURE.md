# Shard Architecture Documentation

## v0.4.0+ Production Updates

This document outlines the key architectural features implemented in Shard v0.4.0 for production readiness.

---

## 1. Golden Ticket Security System

### Overview

The Golden Ticket system provides cryptoeconomic Sybil attack prevention by randomly injecting pre-solved prompts ("Golden Tickets") into the work stream to verify Scout honesty.

### Implementation

**Location**: `desktop/python/golden_ticket.py`

**Key Components**:
- `GoldenTicketGenerator`: Manages ticket injection and verification
- `ScoutReputation`: Tracks per-peer accuracy metrics
- `BanEntry`: Records banned peers with expiration

### Configuration

| Env Variable | Default | Description |
|--------------|---------|-------------|
| `SHARD_GOLDEN_TICKET_RATE` | 0.05 | Injection probability (5%) |
| `SHARD_REPUTATION_THRESHOLD` | 0.70 | Min accuracy to avoid ban |
| `SHARD_MIN_ATTEMPTS_BEFORE_BAN` | 3 | Golden tickets before ban decision |
| `SHARD_BAN_DURATION_HOURS` | 24 | Ban duration |

### Flow

1. Shard randomly decides to inject Golden Ticket (5% by default)
2. Work request includes pre-solved prompt with known answer
3. Scout responds with draft tokens
4. Shard verifies response:
   - Correct: Scout reputation improves
   - Incorrect: Scout reputation degrades
5. If accuracy < 70% after 3+ attempts, Scout is banned

---

## 2. Double-Dip Prevention

### Overview

Prevents GPU OOM crashes by ensuring Scout and Shard don't compete for VRAM on the same machine.

### Implementation

**Location**: `web/src/lib/swarm.ts` (probeLocalShard)

**Detection Logic**:
1. Browser probes `localhost:8000/health`
2. Measures round-trip latency
3. If RTT < 2ms → same machine detected
4. Browser disables WebGPU and routes to local Shard

### Code

```typescript
const LATENCY_THRESHOLD_MS = 2  // Same-machine threshold

async function probeLocalShard(): Promise<LocalShardProbe> {
    const startTime = performance.now()
    const res = await fetch("/health")
    const rttMs = performance.now() - startTime
    
    if (rttMs < LATENCY_THRESHOLD_MS) {
        // Disable WebGPU - route to Shard
        return { available: true, endpoint }
    }
}
```

---

## 3. Mobile Scout Optimization

### Overview

Detects mobile devices and uses appropriately sized models to fit within 4GB RAM constraints.

### Implementation

**Location**: `web/src/lib/webllm.ts`

**Features**:
- Mobile device detection (UA, screen size, device memory)
- Nano model loading (Llama-3.2-1B-q4f16_0) for mobile
- Standard model (Llama-3.2-1B-q4f32_1) for desktop
- Wake Lock API support for background processing

### Detection Criteria

```typescript
function isMobileDevice(): boolean {
    // 1. User agent patterns
    // 2. Screen size < 768px
    // 3. deviceMemory < 4GB
}
```

---

## 4. Privacy Envelope (FHE-Ready)

### Overview

Architecture for future Fully Homomorphic Encryption support. Currently wraps plaintext, structured for encrypted payloads.

### Implementation

**Location**: `desktop/python/privacy_envelope.py`

### Schema

```protobuf
message PrivacyEnvelope {
    string prompt = 1;           // Plaintext (encrypted in future)
    bool requires_tee = 2;       // Request TEE verification
    bool encrypted = 3;          // Encryption status
    string encryption_scheme = 4; // "none" or "fhe"
    string tee_mode = 5;         // "none", "sgx", or "sev"
}
```

### Future: FHE Integration

When ready for production FHE:
1. Replace plaintext with encrypted bytes
2. Add PALISADE/SEAL/HEaan integration
3. Use TEE for verification

---

## 5. Offline-Capable Builds

### Rust Vendor Directory

**Location**: `desktop/rust/vendor/`

All Rust dependencies are vendored for builds without internet access:

```bash
cargo vendor vendor
cargo build --release --offline
```

### Python Lock File

**Location**: `desktop/python/requirements.lock`

Pinned dependencies for reproducible installs:

```bash
pip install -r requirements.lock
```

---

## 6. SQLite Reputation Ledger

### Overview

Production-grade persistence for scout reputations using SQLite.

### Implementation

**Location**: `desktop/python/golden_ticket.py` (SQLiteReputationLedger)

### Schema

```sql
CREATE TABLE scout_reputation (
    peer_id TEXT PRIMARY KEY,
    golden_attempts INTEGER,
    golden_correct INTEGER,
    first_seen REAL,
    last_seen REAL
);

CREATE TABLE banned_scouts (
    peer_id TEXT PRIMARY KEY,
    banned_at REAL,
    ban_duration_hours INTEGER,
    reason TEXT,
    failed_attempts INTEGER
);
```

---

## Testing

### Release Tests

**Location**: `tests/release_test.py`

- `test_symbol_check_shard_rollback_callable`: Verify DLL exports
- `test_boot_test_shard_api_launches`: Smoke test for API
- `test_loop_double_dip_and_remote_gate_contract`: Double-Dip verification

### Running Tests

```bash
# Python tests
pytest tests/ -v

# Frontend build
cd web && npm run build

# Rust build (offline)
cd desktop/rust && cargo build --release --offline
```
