# ADR 0002: Runtime Security Gates

- Status: Accepted
- Date: 2026-02-24

## Context
PoW/auth/replay controls existed but were inconsistent across critical ingress paths.

## Decision
Enforce controls in runtime handlers:
- PoW required on scout work/draft ingress.
- `SHARD_REQUIRE_API_KEY` governs API key requirement.
- `X-Shard-Route: private` always requires API key.
- Signed routes enforce nonce replay protection.

## Consequences
- Improved trust boundary enforcement.
- Stricter client requirements for scout and private paths.

