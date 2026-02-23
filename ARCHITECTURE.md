# Shard Architecture Guide (v0.6.0)

Shard is a distributed P2P AI inference network designed for enterprise privacy and zero-marginal-cost scaling. It leverages **Speculative Decoding** and **1.58-bit Ternary Models** to parallelize LLM inference across heterogeneous hardware.

---

## 1. System Components

### 1.1 The Shard Daemon (`desktop/rust`)
The core P2P sidecar written in Rust. It manages the libp2p swarm and handles all high-integrity logic.
*   **Networking**: Uses `libp2p` with TCP, QUIC, and WebRTC. Implements **AutoNAT** and **DCUtR** for automated NAT traversal and hole punching.
*   **API Gateway**: Provides an OpenAI-compatible HTTP interface for local and remote clients.
*   **Scheduler**: Manages the speculative decoding loop, orchestrating work between browser Scouts and local verifiers.
*   **Ledger**: Tracks **Proof-of-Compute (PoC)** credits using signed receipts to enforce contribution-based rate limiting.

### 1.2 The Scout (`web/src`)
The browser-based execution node. 
*   **Runtime**: Runs in standard web browsers using **WebGPU** via the `web-llm` stack.
*   **Protocol**: Communicates via WebRTC-direct and Gossipsub to pick up `WorkRequest` envelopes and return `ScoutDraft` results.

### 1.3 The Verifier Engine (`cpp/shard-bridge`)
A high-performance C++ bridge to `llama.cpp`.
*   **Quantization**: Optimized for **1.58-bit BitNet** ternary weights, allowing Verifiers to run full-scale models (e.g. 7B-70B) on consumer-grade hardware with extremely low VRAM requirements.
*   **Batching**: Verifies K-tokens from Scouts in a single parallel forward pass.

---

## 2. Trust & Security Model

Shard operates on a **Zero-Trust** P2P architecture:

### 2.1 Cryptographic Envelopes
Every message on the control and data plane is wrapped in a `SignedEnvelope`.
*   **Authentication**: Ed25519 signatures verify the source identity of every draft and request.
*   **Integrity**: SHA-256 hashes ensure payloads aren't tampered with in transit.
*   **Replay Protection**: Nonce-tracking and timestamp-windowing prevent stale message injection.

### 2.2 Probabilistic Verification
Verifiers don't trust Scouts blindly. They perform **Logit Validation**:
1.  Scout submits a sequence of $K$ draft tokens.
2.  Verifier runs the full model on the same context.
3.  Verifier checks if the Scout's tokens match the authoritative model's highest-probability predictions.
4.  Only accepted tokens are appended to the generation; failed tokens trigger autoregressive fallback.

---

## 3. Tokenomics & Incentives

To prevent "freeloading," the network enforces participation-based access:

*   **Proof-of-Compute (PoC)**: When a Verifier accepts work from a Scout, it signs a PoC receipt.
*   **Credit Matrix**: Nodes accumulate these receipts in their local ledger.
*   **Dynamic Rate Limiting**: The API Gateway queries the local ledger. Nodes with high contribution balances are granted enterprise-tier throughput, while inactive nodes are heavily throttled.

---

## 4. Network Topology

Shard uses a hierarchical mesh:
1.  **Bootstraps/Relays**: Cloud-hosted nodes (EC2) that provide stable entry points and circuit relay services for nodes behind symmetric NATs.
2.  **Verifiers**: High-uptime desktop or server nodes that provide authoritative inference.
3.  **Scouts**: Transient browser tabs or mobile devices that provide "burst" compute for speculative drafting.

---

## 5. Development Layout

```text
├── cpp/shard-bridge/      # C++ Inference Engine (BitNet/llama.cpp)
├── desktop/rust/          # Core Daemon (libp2p, Gateway, Scheduler)
│   ├── crates/            # Modularized logic (ledger, metrics, etc.)
│   └── daemon/            # Main binary entry point
├── sdk/python/            # Developer SDK (OpenAI-compatible)
├── web/                   # Web Dashboard & Browser Scout
└── web/src-tauri/         # Desktop GUI wrapper
```
