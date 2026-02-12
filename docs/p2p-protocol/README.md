# Shard P2P Protocol Documentation

Comprehensive documentation for the Shard network's peer-to-peer protocols and message formats.

## Table of Contents

- [Overview](#overview)
- [Protocol IDs](#protocol-ids)
- [Wire Format](#wire-format)
- [Handshake Protocol](#handshake-protocol)
- [Work Distribution](#work-distribution)
- [Gossipsub Topics](#gossipsub-topics)
- [Transports](#transports)
- [Peer Discovery](#peer-discovery)
- [Implementation Notes](#implementation-notes)
- [Examples](#examples)

---

## Overview

The Shard network uses a distributed peer-to-peer architecture built on libp2p for decentralized inference. The system coordinates three types of nodes:

| Node Type | Name | Hardware | Role |
|-----------|------|----------|------|
| **A** | **Oracle** | Desktop/Server with GPU | Full model host, verifies draft tokens |
| **B** | **Scout** | Browser (WebGPU) | Runs tiny draft model, submits token guesses |
| **C** | **Leech** | Any | Pure consumer, queued behind contributors |

### Key Principles

1. **Verify, Don't Trust**: All Scout drafts are verified by Oracles using the full model
2. **Heavier is Truth**: GPU nodes always override browser drafts in case of discrepancies
3. **Distributed Inference**: Work is distributed across the network for scalability
4. **Golden Ticket Security**: Automated verification to prevent Sybil attacks

---

## Protocol IDs

libp2p uses path-based protocol identifiers with version numbers for compatibility and rollback.

### Format

```
/namespace/component/version
```

### Example Protocol IDs

| Protocol | Description | Usage |
|----------|-------------|-------|
| `/shard/1.0.0/handshake` | Initial peer connection | All new connections |
| `/shard/1.0.0/verify` | Draft verification | Oracle-only |
| `/shard/1.0.0/metrics` | Health checks | Both Oracles & Scouts |
| `/shard/1.0.0/protocol-list` | Protocol discovery | All nodes |
| `/multistream/1.0.0` | Protocol negotiation | Connection setup |

### Protocol Versioning

- **Major version** (X.0.0): Breaking changes
- **Minor version** (X.Y.0): Backward compatible additions
- **Patch version** (X.Y.Z): Bug fixes

Always increment the appropriate version number when making changes to protocol wire formats.

---

## Wire Format

All protocol messages over the P2P network use a consistent binary serialization format.

### Message Encoding

Every message consists of:

```
[message_length (varint)] + [protobuf_message_bytes]
```

**Fields**:
- `message_length`: Unsigned variable-length integer (up to 9 bytes)
- `protobuf_message_bytes`: Actual Protocol Buffer message data

### Example

```hex
# Message for work assignment
# Length: 23 bytes (0x17)
# Content: ScoutRequest protobuf
17 0a 1a 01 39 0a 03 41 42 43 2a 08 0a 1a 02 31 32 33
```

### Size Constraints

| Message Type | Max Size | Constraints |
|--------------|----------|-------------|
| Handshake | 1 KiB | Maximum identifier exchange |
| Work Request | 8 KiB | Prompt context (max 8KB) |
| Work Response | 64 KiB | Draft tokens + metadata |
| Gossipsub messages | 2 MiB | Message batching limit |

**Buffer overflow protection**: All implementations MUST reject messages exceeding the maximum size.

---

## Handshake Protocol

The handshake establishes a secure connection between peers and exchanges capability information.

### Lifecycle Stages

1. **Transport Layer**: Connection established (TCP/WebSocket/WebRTC)
2. **Protocol Negotiation**: Multistream-select protocol
3. **Authentication**: Noise/TLS encryption
4. **Capability Exchange**: Peer identity and capabilities
5. **Multiplexing**: Protocol multiplexer (yamux) setup

### Protocol Sequence

```
┌─────────┐     ┌─────────┐
│ Client  │     │ Server  │
└─────┬────┘          └────┬────┘
      │                    │
      │ 1. Send "Handshake"  │
      │    protocol request  │
      v                    v
      │                    │
      v 2. Verify protocol  │
      v    support response  │
      │                    │
      v 3. Exchange keys    │
      v    (Noise/TLS)      │
      │                    │
      v 4. Send Handshake   │
      v    message         │
      │                    │
      │ 5. Verify signature │
      │    and accept       │
      │                    │
      v                    v
      v 6. Start multi-    │
      v    stream muxing   │
      │                    │
      v                    v
      v 7. Begin protocol  │
      v    communication   │
      │                    │
      └────────────────────┘
```

### Multistream Negotiation

The handshake begins with multiselect protocol negotiation:

```
Client → Server: /multistream/1.0.0
Server → Client: /multistream/1.0.0  (echoed)
Client → Server: /shard/1.0.0/handshake
Server → Client: /shard/1.0.0/handshake  (agreed) or "na"
```

**Failure handling**:
- If server responds with `"na"`, the peer must try alternative protocols
- If no protocol is available, the connection is terminated

### HandshakeMessage

```protobuf
syntax = "proto3";

message HandshakeMessage {
  // Protocol version we support
  string protocol_version = 1;
  
  // Node type (Oracle/Scout/Leech)
  NodeType node_type = 2;
  
  // Capabilities (list of supported protocol IDs)
  repeated string supported_protocols = 3;
  
  // Hardware info (optional)
  optional HardwareInfo hardware_info = 4;
  
  // Public key for identity verification
  bytes public_key = 5;
}

enum NodeType {
  NODE_TYPE_UNSPECIFIED = 0;
  NODE_TYPE_ORACLE = 1;   // Full model host
  NODE_TYPE_SCOUT = 2;    // Browser draft generation
  NODE_TYPE_LEECH = 3;     // Consumer-only
}

message HardwareInfo {
  string gpu_model = 1;       // e.g., "NVIDIA RTX 3080"
  int32 vram_gb = 2;         // VRAM capacity in GB
  bool webgpu_enabled = 3;    // Can use WebGPU
  string npu_model = 4;      // Neural processing unit (optional)
  int32 cpu_cores = 5;        // CPU core count
}
```

### Key Exchange (Noise Protocol)

Uses Noise Protocol Framework with IK pattern for key exchange:

**Step 1**: Both peers generate DH (Diffie-Hellman) keys
**Step 2**: Send Hello messages with their DH public keys
**Step 3**: Derive shared secret and encrypt all subsequent messages

**Cipher suite**: X25519 for key exchange + ChaCha20-Poly1305 for encryption

**Implementation**: Use libp2p's built-in Noise support for proper implementation.

---

## Work Distribution

### Overview

Work distribution coordinates tasks between Oracles and Scouts using gossipsub pub/sub protocol.

### Publisher-Subscriber Model

Oracles **publish** work requests to the `shard-work` topic. Scouts **subscribe** to receive and process work.

### Work Assignment Lifecycle

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  Oracle  │    │ Control  │    │  Scout   │    │   Oracle  │
│          │    │  Plane   │    │          │    │  (Verifier)│
└────┬─────┘    └────┬─────┘    └────┬─────┘    └─────┬─────┘
     │                │                │                │
     │ 1. Broadcast    │                │                │
     │    work request │                │                │
     │    (gossipsub)  │                │                │
     v                v                v                v
     │                │    2. Receive  │                │
     │                │    work       │                │
     │                │                v                │
     │                │    3. Generate  │                │
     │                │    draft tokens│                │
     │                │                v                │
     │                │    4. Submit   │                │
     │                │    result      │                │
     │                │                v                │
     │                │    5. Verify   │                │
     │                │    against     │                │
     │                │    full model  │                │
     │                │                │                │
     │                │                │                6. Final
     │                │                │                │    response
```

### Message Formats

#### WorkAssignment (published by control plane)

```protobuf
syntax = "proto3";

message WorkAssignment {
  bytes prompt_cid = 1;          // Content-addressed ID of work
  int32 max_tokens = 2;          // Maximum tokens to generate
  int32 priority = 3;            // Normalized priority (1-10)
  bytes model_cid = 4;            // Target model specification
  int64 timeout_ms = 5;          // Request timeout (milliseconds)
  string node_type = 6;          // Target node type (for routing)
}
```

#### WorkResult (submitted by Scouts)

```protobuf
message WorkResult {
  bytes work_cid = 1;             // Matching work assignment CID
  bytes result_cid = 2;            // Content-addressed result
  repeated uint32 token_ids = 3;     // Generated token sequence
  bool verified = 4;                 // Oracle verification status
  bytes proof = 5;                   // Proof of inference (signature)
  int32 inference_time_ms = 6;      // Time to generate (milliseconds)
  float confidence_score = 7;       // Scout confidence (0-1)
}
```

### Priority-Based Routing

Oracles can set priorities (1-10, where 1 is highest):

| Priority | Use Case | Expected Latency |
|----------|----------|-----------------|
| 1 | Critical work (Golden Tickets) | < 100ms |
| 2-3 | Real-time generation | 100-500ms |
| 4-7 | Standard work | 500-1000ms |
| 8-10 | Background work | 1000-2000ms |

---

## Gossipsub Topics

### Topic: shard-work

**Purpose**: Distribution of work assignments to Scouts

**Direction**: Published by Control Plane → Subscribed by Scouts

**Message Schema**:
```protobuf
message GossipWorkMessage {
  WorkAssignment assignment = 1;
  repeated bytes prior_work_ids = 2;  // Work already known by peer
  uint32 ttl = 3;                       // Time-to-live in seconds
}
```

**Publishing Rules**:
- Must use gossipsub's Gossipsub feature set for flood-sub
- Messages must be signed by the publisher (Oracle)
- Use quality-of-service (QoS) for important work
- Set appropriate heartbeat interval (default: 3 seconds)

### Topic: shard-work-result

**Purpose**: Distribution of verified inference results

**Direction**: Published by Oracles → Subscribed by Leech nodes (read-only)

**Message Schema**:
```protobuf
message ResultBroadcast {
  bytes work_cid = 1;
  WorkResult result = 2;
  bytes verification_signature = 3;  // Oracle's signature
  int32 confidence_score = 4;
  int32 proof_strength = 5;          // Oracle confidence (0-1)
}
```

**Publishing Rules**:
- Only Oracles can publish to this topic (security requirement)
- Results must include proof of inference (cryptographic signature)
- Use high QoS (priority 10) for critical results
- Set expiration (TTL) to prevent message replay

### Topic: shard-topology

**Purpose**: Regular topology updates and peer discovery

**Direction**: Published by all nodes → Subscribed by all nodes

**Message Schema**:
```protobuf
message TopologyMessage {
  repeated NodeInfo nodes = 1;
  uint32 timestamp = 2;                 // Unix timestamp
  repeated AddressInfo listen_addrs = 3; // Available addresses
}
```

**Publishing Rules**:
- All nodes should publish periodically (every 30 seconds)
- Use gossipsub's TRSM (Topic Registration using Signed Messages)
- Use bounding set to limit scope (bootstrap node + local peers only)

---

## Transports

### TCP Transport

**Multiaddr Format**: `/ip4/<ip-address>/tcp/<port>`

**Properties**:
- Connection-oriented
- Reliable delivery
- Works through most firewalls/NAT
- Higher latency than direct WebRTC

**Use Case**:
- Reliable P2P connections between nodes on same network
- NAT traversal with Bootstrap peers
- Default transport for production nodes

**Implementation**:
```rust
use libp2p::tcp::TcpTransport;
let transport = TcpTransport::new(tokio_util::net::UnixSocketAddr::maybe_uds(...));
```

### WebSocket Transport

**Multiaddr Format**: `/dns4/<domain>/tcp/<port>/ws` or `/ip4/<ip-address>/tcp/<port>/ws`

**Properties**:
- Message-oriented (message frames)
- Full-duplex communication
- Lower overhead than HTTP polling
- Browser-friendly
- Works through HTTP proxies

**Use Case**:
- Secure connections through corporate firewalls
- Browser-to-server connections (WebSockets)
- Hybrid P2P networking (TCP + WebSocket)

**Implementation**:
```rust
use libp2p::websocket::WsConfig;
let ws = WsConfig::new(...)
```

### WebRTC Transport

**Multiaddr Format**: `/ip4/<ip-address>/udp/<port>/webrtc-direct/p2p/<peer-id>`

**Properties**:
- Browser-native for direct peer-to-peer
- NAT traversal with ICE/STUN/TURN
- DTLS for security
- SRTP for data channels

**Use Case**:
- Browser Scouts connecting to Oracles
- Direct browser-to-browser connections
- Zero-latency connections when firewalls allow

**Implementation**:
```rust
use libp2p::webrtc::WebRTCConfig;
let webrtc = WebRTCConfig::new(...)
```

### Transport Recommendations

**For Oracles (desktop)**:
- **Primary**: TCP with mTLS
- **Secondary**: WebSocket for enterprise networks
- **Optional**: WebRTC for mobile remote access

**For Scouts (browser)**:
- **Primary**: WebRTC-direct
- **Secondary**: WebSocket (fallback)
- **Tertiary**: TCP (if WebRTC blocked)

**For Leech nodes**:
- **Primary**: WebSocket (best compatibility)
- **Secondary**: HTTP polling (if WebRTC blocked)

---

## Peer Discovery

### Kademlia DHT

The Shard network uses Kademlia-based DHT for peer discovery and content routing.

#### DHT Operations

**Find Peer** (FIND_VALUE):
- Find a specific peer by peer ID
- Uses XOR distance for routing
- Returns the closest peers to target

**Find Providers** (FIND_NODE):
- Find peers that can provide a content key
- Used to discover nodes with specific capabilities

#### DHT Routing Table

**Structure**: K-buckets (k = log₂(256) = 8)

**Properties**:
- XOR distance metric
- Periodic refresh (every hour)
- Bootstrap refresh (every 24 hours)

**Bootstrap Nodes**:
```
/ip4/1.2.3.4/tcp/4001/p2p/QmBootstrap...
/ip4/2.3.4.5/tcp/4001/p2p/QmBootstrap...
```

**Configuration**:
```rust
use libp2p::kad::store::MemoryStore;
use libp2p::kad::store::Store;

let store = MemoryStore::new();
let routing_table = KademliaRoutingTable::new(store, network_key)?;
```

### Bootstrap Mechanisms

**Initial Discovery**:
1. Scout requests topology from Oracle
2. Oracle returns list of available peers
3. Scout connects to bootstrap peers
4. Scout announces itself via gossipsub

**Automatic Reconnection**:
- Scouting reconnects if connection lost
- Uses bootstrap list + DHT to find new peers
- Drops stale connections after 5 minutes of no activity

**MDNS** (optional):
- Local peer discovery via multicast DNS
- Used for LAN deployments
- Complements DHT with zero-config discovery

---

## Implementation Notes

### Error Handling

**Error Codes** (from libp2p):

| Code | Name | When to Use |
|------|------|-------------|
| 1 | CANCELLED | Operation cancelled by client |
| 3 | INVALID_ARGUMENT | Invalid parameters |
| 5 | NOT_FOUND | Resource doesn't exist |
| 7 | PERMISSION_DENIED | Insufficient permissions |
| 13 | INTERNAL | Internal server error |
| 14 | UNAVAILABLE | Service temporarily unavailable |
| 16 | UNAUTHENTICATED | Missing authentication |
| 17 | ALREADY_EXISTS | Resource already exists |

**Custom Error Handling**:
```protobuf
message Ack {
  bool ok = 1;
  string detail = 2;
  repeated ErrorDetail errors = 3;
}

message ErrorDetail {
  string code = 1;
  string message = 2;
  string field = 3;  // Optional field name
}
```

### Security Considerations

**Encryption**:
- All connections MUST be encrypted (Noise or TLS 1.3+)
- Peer IDs must be verified via public keys
- All gossipsub messages MUST be signed

**Authentication**:
- Oracles verify scout signatures on work results
- Scout signatures verify oracle identity
- Mutual authentication (both sides verify each other)

**Rate Limiting**:
- Scout endpoints: 120 requests/min per IP
- Oracle endpoints: 60 requests/min per peer ID
- Gossipsub: Message rate limiting (1000 msgs/sec)

**DoS Prevention**:
- Message size limits (enforce max size constraint)
- Rate limiting per peer ID
- Rejection of malformed messages
- Connection timeouts (30 seconds default)

### Performance Optimization

**Message Batching**:
- Batches multiple work requests into single gossipsub messages
- Reduces network overhead
- Increases throughput

**Congestion Control**:
- Uses gossipsub's built-in congestion control
- Adapts to network conditions
- Prevents network congestion

**Backpressure**:
- Implements message queuing at application layer
- Prevents memory overflow on slow peers
- Dropping old messages before new ones

**Connection Pooling**:
- Reuses established connections
- Reduces connection overhead
- Improves latency

---

## Examples

### Complete Work Flow Example

**Scenario**: Oracle broadcasts a work assignment to Scouts

**Step 1: Oracle broadcasts work**
```protobuf
// Control plane publishes to shard-work topic
message GossipWorkMessage {
  WorkAssignment {
    prompt_cid: "cid-abc-123"
    max_tokens: 256
    priority: 5
    model_cid: "model-shard-hybrid"
    timeout_ms: 30000
    node_type: "scout"
  }
  prior_work_ids: []
  ttl: 300  // 5 minutes
}
```

**Step 2: Scout receives and processes**
```protobuf
// Scout generates draft tokens using WebLLM
message WorkResult {
  work_cid: "cid-abc-123"
  result_cid: "cid-response-789"
  token_ids: [1001, 1002, 1003, 1004]
  verified: false
  proof: "<signature>"
  inference_time_ms: 450.0
  confidence_score: 0.95
}
```

**Step 3: Oracle verifies and publishes result**
```protobuf
// Oracle verifies tokens against full model
// If correct, publishes to shard-work-result topic
message ResultBroadcast {
  work_cid: "cid-abc-123"
  result: WorkResult { ... }
  verification_signature: "<oracle-signature>"
  confidence_score: 1.0
  proof_strength: 0.99
}
```

**Step 4: Leech nodes consume results**
- Leech nodes subscribe to `shard-work-result` topic
- Consume verified results for inference
- No verification required (pre-verified by Oracles)

### Golden Ticket Verification Example

**Scenario**: Control plane injects Golden Ticket

**Golden Ticket Definition**:
```protobuf
message GoldenTicket {
  bytes work_cid = 1;
  string prompt = 2;        // Pre-solved prompt
  string correct_answer = 3; // Known answer
  string expected_tokens = 4; // Expected token sequence
}
```

**Scout Response**:
```protobuf
message WorkResponse {
  draft_tokens: ["The", " quantum", " physics"]
  // Scout's tokens don't match expected
}
```

**Oracle Verification**:
- Compares draft tokens against expected tokens
- Verifies correctness
- If correct: Accepts result, reputation +1
- If incorrect: Rejects result, reputation -1
- If consistently incorrect: Bans scout

---

## References

### libp2p Documentation

- [libp2p Specs](https://github.com/libp2p/specs) - Protocol specifications
- [libp2p Documentation](https://docs.libp2p.io) - User documentation
- [Kademlia DHT](https://github.com/libp2p/specs/blob/master/kad-dht/README.md) - DHT implementation
- [Gossipsub](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/README.md) - Pub/sub protocol

### Security

- [Noise Protocol Framework](https://noiseprotocol.org/) - Cryptographic handshake
- [X25519](https://datatracker.ietf.org/doc/html/rfc7748) - Curve25519 ECDH
- [RFC 8594](https://tools.ietf.org/html/rfc8594) - HTTP Sunsetting header

### Industry Examples

- [IPFS Spec](https://specs.ipfs.tech/bitswap-protocol/) - P2P data exchange
- [Filecoin Spec](https://docs.filecoin.io/reference/systems/filecoin_nodes/network/) - Filecoin network protocols

---

**Last Updated**: 2026-02-12  
**Version**: 1.0  
**Status**: Active
