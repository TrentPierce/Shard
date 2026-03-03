# Credit Economy

## Earning Rate
- Accepted draft token reward: `0.001 Shards` per accepted token.
- Reward only applies to verifier-accepted draft tokens.
- Rejected tokens earn `0`.

## Hourly Wallet Cap
- Maximum earnable credits per wallet per hour: `25.0 Shards`.
- Implemented as a sliding 1-hour window over reward events.
- If a new reward would exceed the cap, the reward event is rejected.

## Minimum Acceptance Threshold
- A wallet must maintain at least `40%` acceptance over its last `100` submissions.
- If acceptance is below `40%`, wallet is quality-ineligible and receives no new credits.
- Exactly `40%` is eligible.

## Credit Expiry
- Credits expire after `90 days` of wallet inactivity.
- Inactivity is defined as no accepted-credit events and no spend events.
- Expiry runs as periodic ledger maintenance.

## PoW Requirement
- New earning wallets must pass minimum PoW difficulty of `16` leading zero bits.
- Wallets without valid PoW verification cannot earn credits.

## Quality Score
- Quality score is the weighted outcome of:
  - Acceptance rate over last 100 submissions.
  - Golden-ticket audit score.
  - Recent verifier disagreement penalties.
- Payout multiplier bands:
  - `>= 0.90` score: `1.0x`
  - `0.70 - 0.89`: `0.75x`
  - `0.40 - 0.69`: `0.5x`
  - `< 0.40`: ineligible (`0x`)

## Anti-Sybil Policy
- Subnet rule: max `5` newly-registered wallets per `/24` subnet per hour.
- Exceeding subnet cap flags wallets for audit and blocks earnings.
- Each sybil flag is written as structured JSON to `shard_audit.jsonl`.
