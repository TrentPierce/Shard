# Benchmark Completion Roadmap

Date: March 6, 2026
Scope: finish the verifier/scout benchmark program without publishing misleading numbers.

## Goal

Get to a state where:

- `short_rc_stability` remains a truthful release gate.
- `long_scout_generation` becomes a truthful scout-uplift gate.
- public benchmark claims are backed by repeatable artifacts.

## Current Facts

- Short RC stability is the only benchmark class that currently produces a reliable ship/no-ship signal.
- The late-draft / wrong-owner bug in the speculative path was real and is now fixed.
- Long-run scout benchmarks still fail because the verifier runtime profile is too tight for sustained 64-token traffic.
- The current long benchmark profile is reusing constraints that were chosen for short benchmark protection, not long-generation throughput.

## Phase 1: Separate Long-Run Runtime Profile

Status: complete

Problem:
- `deploy/release/benchmark.env` is trying to serve two jobs:
  - protect the short RC release gate
  - support long 64-token scout uplift runs
- those goals conflict

Work:
- create a dedicated `deploy/release/long_benchmark.env`
- keep `deploy/release/benchmark.env` focused on short RC stability
- teach local and EC2 redeploy scripts how to select:
  - `benchmark.env` for short RC
  - `long_benchmark.env` for long scout runs
- document the split in the release runbook

Exit criteria:
- operators can deploy the short and long benchmark profiles intentionally
- profile selection is explicit in scripts and docs
- no more “one env file for both benchmark classes”

## Phase 2: Long-Matrix Harness Defaults

Status: complete

Problem:
- `long_scout_generation` still launches with fixed stress assumptions that collapse the baseline before scout uplift can be judged

Work:
- make long-matrix defaults choose safer calibration behavior than short RC
- prefer auto-calibrated rate for long-class runs
- encode long-class defaults for:
  - duration
  - request timeout
  - readiness tolerance
  - browser warmup behavior
- make reports state which runtime profile and class defaults were used

Exit criteria:
- long matrix launches under its own assumptions instead of inheriting short RC expectations
- reports clearly show whether the run used calibrated load
- repeated long runs are comparable without manual note-taking

## Phase 3: Clean Long No-Scout Baseline

Status: complete

Problem:
- if long no-scout runs already fail, scout uplift claims are meaningless

Work:
- deploy the dedicated long profile on local and EC2
- rerun:
  - `one-node-no-scouts`
  - `two-node-no-scouts`
- tune verifier queue cap / queue wait for long runs until the long no-scout baseline is clean enough to trust

Exit criteria:
- long no-scout runs complete with low or zero `503` collapse
- p95 and error rate become baseline measurements, not admission-failure artifacts
- only after that do scout comparisons become actionable

Current result:
- `one-node-no-scouts` on `long_benchmark.env`: `928.404 ms` p95, `2.0167 TPS`, `0%` errors
- `two-node-no-scouts` on `long_benchmark.env`: `937.711 ms` p95, `2.0167 TPS`, `0%` errors
- the long verifier-only baseline is no longer admission-limited at the default `2.0 rps`

## Phase 4: Honest Long Scout Uplift

Problem:
- long scout runs still need to prove measured-window speculative engagement and net improvement

Work:
- rerun repeated `long_scout_generation` on the cleaned long profile
- inspect:
  - speculative samples
  - accepted draft tokens
  - p95 versus no-scout baseline
  - TPS versus no-scout baseline
- only update website / README if the repeated long matrix is both:
  - clean
  - favorable

Exit criteria:
- `long_scout_generation` either earns `GO` honestly or remains `NO_GO` honestly
- public benchmark numbers are updated only from repeatable long-run artifacts

## Order Of Execution

1. Phase 1: add the dedicated long benchmark profile and profile selection support
2. Phase 2: make long-matrix defaults match the long profile
3. Phase 3: establish a trustworthy long no-scout baseline
4. Phase 4: rerun repeated long scout uplift and decide whether public numbers change

## Non-Goals

- Do not change public benchmark claims before the long matrix is repeatable.
- Do not optimize scouts further while the long verifier baseline is still admission-limited.
- Do not collapse short RC and long scout benchmarks back into one report.
