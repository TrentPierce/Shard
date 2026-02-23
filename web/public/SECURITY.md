# Security Policy

## Reporting a Vulnerability

If you discover a security issue, please report it privately to **security@shardnetwork.live**.

Please include:
- A clear description of the issue.
- Reproduction steps or proof-of-concept details.
- Affected components/versions.
- Any suggested mitigations.

We will acknowledge receipt as quickly as possible and coordinate remediation before public disclosure.

## Disclosure Policy

- Do **not** open public GitHub issues for exploitable vulnerabilities.
- We prefer coordinated disclosure after a fix or mitigation is available.
- We may request additional validation details during triage.

## Security Scope (Shard Guarantees)

The project currently emphasizes the following controls:

- **PoW gating** for ingress and mesh routes to increase abuse costs.
- **Probabilistic MatMul verification** for scout draft validation against verifier outputs.
- **Private routing support (`X-Shard-Route: private`)** for trusted-network inference paths.

## Out of Scope

The following are currently out of scope for bounty-style guarantees:

- Vulnerabilities caused only by unsupported third-party forks.
- Local misconfiguration of operator infrastructure (DNS, firewalls, host hardening).
- Social engineering, phishing, or leaked credentials outside this repository.
- Denial-of-service claims without reproducible protocol-level bypass of PoW controls.

## Supported Versions

Security fixes are prioritized for the latest release on `main`.
