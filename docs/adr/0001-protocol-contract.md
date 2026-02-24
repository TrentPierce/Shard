# ADR 0001: Canonical Protocol Contract

- Status: Accepted
- Date: 2026-02-24

## Context
Protocol drift existed across web, daemon, and SDK surfaces for chat/scout routes.

## Decision
Use canonical JSON schemas under `docs/schemas/` and enforce contract checks in CI tests.

## Consequences
- Reduces incompatible payload regressions.
- Requires schema updates for any contract change.

