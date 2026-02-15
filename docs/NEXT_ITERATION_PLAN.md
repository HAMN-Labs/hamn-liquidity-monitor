# Next Iteration Plan

## Goal
Increase real detection quality on Arbitrum mainnet data and harden runtime behavior for long-running operation.

## Phase 1: Real DeFi Coverage
- Select high-activity block ranges with real DEX logs (Uniswap/Sushiswap/Camelot style pools).
- Add configurable topic filters for known liquidity-related events.
- Run replay on selected ranges and measure:
  - extracted features per block,
  - memory pattern creation/match ratio,
  - sequence transition density.

## Phase 2: Feature Quality Improvements
- Extend feature parsing for protocol variants where event payload format differs.
- Add token-level normalization helpers (decimals-aware volume normalization).
- Add edge-case handling for malformed or partial logs.
- Add unit tests from real anonymized receipt samples.

## Phase 3: Detection Quality Baseline
- Define precision/recall proxy metrics for:
  - swap detection,
  - add/remove liquidity detection.
- Build a small labeled validation set (manual labeling for sample transactions).
- Introduce acceptance thresholds for each metric.

## Phase 4: Runtime Hardening
- Add periodic health heartbeat output in follow mode.
- Add graceful shutdown handling with final metrics flush.
- Add optional periodic snapshot save for memory state.
- Add configurable fail-fast / fail-soft mode for RPC errors.

## Phase 5: Observability and Operations
- Add structured log mode (JSON output).
- Export runtime metrics to a machine-readable sink (stdout JSON lines).
- Add minimal runbook:
  - startup checklist,
  - common failures and responses,
  - parameter tuning hints.

## Phase 6: Packaging
- Add release profile guidance and binary packaging instructions.
- Add sample systemd service config for continuous operation.
- Add command presets for:
  - replay mode,
  - near-real-time mode.

## Exit Criteria
- Stable runtime in follow mode for at least 12 hours without manual restarts.
- Non-zero feature extraction on selected DeFi-heavy ranges.
- Memory and sequence metrics show meaningful activity:
  - non-zero active patterns,
  - non-zero transitions,
  - stable churn below agreed threshold.
