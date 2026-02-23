# Self-Healing P2P Network Implementation

## Current Architecture Analysis

### What's Already Implemented (lines in main.rs):

1. **Bootstrap Sources** (priority order):
   - `--bootstrap_node` CLI args (line 1518)
   - `--bootstrap_file` CLI args (line 1511-1515)
   - Persisted peers from `known_peers.json` (line 1516)
   - `SHARD_DEFAULT_BOOTSTRAP` env var (lines 1521-1529)
   - Hardcoded fallback: EC2 35.175.242.222 (lines 1541-1543)

2. **Kademlia DHT** is already enabled:
   - `kad: KadBehaviour<MemoryStore>` (line 1777)
   - `kad.bootstrap()` called at startup (line 1913)
   - Peers added to DHT: `kad.add_address(&peer_id, addr)` (line 1907)

3. **Peer Tracking**:
   - `PeerInfo` struct: `peer_id`, `connected_at`, `last_seen_at`, `addrs`, `verified`, `handshake_failures` (lines 388-395)
   - `peers: Arc<Mutex<HashMap<String, PeerInfo>>>` (line 1603)
   - `known_peers: Arc<Mutex<Vec<String>>>` (line 1604)

---

## Implementation Plan

### Phase 1: Bootstrap Discovery Service (Simplest)

**Goal**: Query a well-known endpoint for bootstrap peers, fallback to hardcoded.

1. Add new CLI flag: `--bootstrap-url`
2. Fetch JSON from URL on startup: `[{"peer_id": "...", "multiaddr": "..."}]`
3. Merge with existing bootstrap list
4. Add periodic refresh (every 5 minutes)
5. Cache results locally

### Phase 2: Peer Stability Tracking

**Goal**: Track which peers are stable and can become bootstraps.

1. Add stability metrics to `PeerInfo`:
   - `first_seen_at`: u128 (when first connected)
   - `consecutive_uptime_ms`: u128 (total uptime since first seen)
   - `successful_handshakes`: u32
   - `failed_handshakes`: u32
   - `avg_latency_ms`: f32
   - `is_stable`: bool (calculated)

2. Stability criteria (all must be true):
   - Uptime > 1 hour
   - Successful handshakes >= 3
   - Failure rate < 10%
   - Latency < 5000ms

### Phase 3: Self-Advertising as Bootstrap

**Goal**: Stable nodes advertise themselves as bootstraps.

1. Add new CLI flag: `--advertise-bootstrap` (default: true if stable)
2. When peer becomes stable, add to local "bootstrap candidates"
3. Periodically POST to discovery endpoint with own info:
   ```json
   {
     "peer_id": "12D3...",
     "multiaddr": "/ip4/x.x.x.x/tcp/4001/p2p/...",
     "stability_score": 95,
     "uptime_hours": 24,
     "version": "0.5.0"
   }
   ```

### Phase 4: Kademlia DHT Bootstrap Storage

**Goal**: Use DHT to share bootstrap list without centralized endpoint.

1. Store known good bootstraps in DHT:
   - Key: `/shard/bootstrap/peers`
   - Value: JSON array of bootstrap peer info

2. On startup:
   - Query DHT for bootstrap peers
   - Merge with other sources

---

## File Changes Required

### 1. CLI Args (lines ~100-200)
- Add `--bootstrap-url <URL>` flag
- Add `--advertise-bootstrap` flag
- Add `--stability-threshold-hours` flag

### 2. PeerInfo Struct (lines ~388-395)
- Add stability fields

### 3. SharedState (lines ~1588-1676)
- Add bootstrap candidates storage
- Add discovery client

### 4. New Module: bootstrap_discovery.rs
- Fetch from HTTP endpoint
- Parse and merge bootstrap lists
- Periodic refresh

### 5. API Endpoint: /v1/system/bootstrap (NEW)
- GET: Returns current bootstrap list
- POST: Register as bootstrap candidate

### 6. Stability Calculation
- In peer connection handler
- Calculate when peer state changes

### 7. Discovery Advertisement
- When stable, POST to discovery endpoint
- Include stability metrics

---

## Implementation Priority

1. **First**: Add `--bootstrap-url` and HTTP fetching (lowest risk)
2. **Second**: Add stability tracking to existing PeerInfo
3. **Third**: Add self-advertisement when stable
4. **Fourth**: Use DHT for distributed bootstrap storage

---

## Backward Compatibility

- All new features are additive
- Existing bootstrap mechanism works unchanged
- Hardcoded EC2 fallback remains as last resort
- No breaking changes to API

---

## Testing Strategy

1. Start with single node, verify connects to EC2
2. Add second node, verify both see each other
3. Add discovery endpoint mock
4. Verify nodes register as bootstraps after stability threshold
5. Kill EC2, verify network continues working
6. New node joins via other bootstraps
