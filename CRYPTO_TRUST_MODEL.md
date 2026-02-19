# Crypto Trust Model Audit (Ed25519-Centric)

## Design Intent
Shard should be a zero-trust distributed inference infrastructure where every participating node is cryptographically identifiable and every critical exchange is verifiable.

## What Is Implemented Today

## 1. Node Identity (Implemented)
- Node keypair is Ed25519 (`desktop/rust/src/crypto/identity.rs`).
- Secret key persisted locally in `identity.json`.
- Public key hex is exposed as current "wallet" string (should be renamed node public key / node identity).
- Same key material is adapted into libp2p identity keypair.

Security properties:
- Stable cryptographic node identifier.
- Transport identity tied to node key.

## 2. Key Backup/Recovery (Implemented)
- Backup/import/verify flows in `desktop/rust/src/crypto/wallet_backup.rs`.
- Encrypted envelope uses Argon2id KDF + XChaCha20Poly1305.
- Backup integrity includes pubkey cross-check.

Security properties:
- Encrypted key portability with password-based protection.

## 3. Signed Ledger Transactions (Implemented)
- Reward/credit tx signed with Ed25519 (`LedgerState::sign_reward_tx`).
- Signature, signer pubkey, and nonce checked in `LedgerState::apply_signed_tx`.
- Prevents stale nonce replay per signer.

Security properties:
- Authenticated ledger mutation path.
- Replay resistance for signed ledger tx stream.

## What Is Not Yet Enforced (Critical Gaps)

## 4. Job Dispatch Signatures (Missing)
- Work requests over gossipsub/request-response are not universally wrapped in signed envelopes.
- Receivers do not uniformly require signature verification before queueing execution work.

Risk:
- Forged or tampered work assignment injection.

## 5. Result Signatures (Missing)
- Scout/job results are not uniformly signed by node private keys with strict verification before acceptance.
- Current filtering is mostly reputation/blackhole and shape/timing checks.

Risk:
- Spoofed result submissions and identity impersonation.

## 6. Handshake vs Authenticated Session (Partial)
- Handshake ping/pong exists, and peers are marked verified, but not all higher-level message acceptance is bound to a verified cryptographic session state.

Risk:
- Trust escalation from weak signals (connectivity) to strong authorization without sufficient cryptographic binding.

## 7. Naming and Trust Semantics (Needs Correction)
- "wallet" terminology appears in API/state/CLI for node identity.
- This creates accidental token/economics semantics and weakens clarity of trust boundaries.

Required renames:
- `wallet` -> `node_public_key` or `node_identity`
- `wallet_address` -> `peer_key`/`identity_key`
- `wallet backup` -> `node key backup`

## Target Trust Model (Required)

Every control-plane message should carry:
1. `node_pubkey`
2. `timestamp` / monotonic nonce
3. canonical payload hash
4. signature over canonical payload envelope

Acceptance policy:
1. Reject if key is unknown/unregistered.
2. Reject if signature invalid.
3. Reject if nonce/timestamp stale or replayed.
4. Reject if peer identity does not match transport/session mapping.
5. Only then enqueue work or accept result.

## Identity-Bound Reputation
Reputation must be keyed to Ed25519 public key, not mutable scout string IDs.

Required controls:
- reputation record keyed by node pubkey
- penalties and recovery tied to key identity
- key rotation policy with explicit migration semantics

## Minimal Cryptographic Contract for Phase 1-3
- Mandatory signed registration message on startup.
- Mandatory signed heartbeat every N seconds.
- Mandatory signed work dispatch and signed result return.
- Mandatory signature verification before result acceptance and before reputation mutation.
- Signed fallback execution events recorded for auditability.

## Threat Model Summary
Protected now:
- local key secrecy (if host secure)
- ledger tx authenticity/integrity

Not fully protected yet:
- end-to-end authenticity of work/result plane
- replay-safe authenticated execution lifecycle
- strict peer auth gating for all critical operations

Conclusion:
Ed25519 foundation is present and viable, but trust enforcement is still partial. Zero-trust claims require signed control/data-plane envelopes and strict verification gates across all node interactions.
