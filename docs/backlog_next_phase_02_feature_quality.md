# Next Iteration Backlog: Phase 2 (Feature Quality Improvements)

## Scope
Extend feature extraction quality for protocol/event variants.

## Tasks
- [x] Add topic0 discovery output (`topic0_top`) for parser expansion prioritization.
- [x] Add support for additional event signatures beyond current Swap/Mint/Burn parser assumptions.
- [x] Add token-decimals-aware normalization helpers.
- [x] Improve malformed/partial log handling with explicit counters.
- [x] Add real-world receipt fixtures for unit tests.

## Notes
- Discovery on `359066951..359066952` produced top topic0 signatures for next parser targets.
- Added swap-like support for:
  - `0xc42079f9...` (UniswapV3-style swap topic),
  - `0x19b47279...` (swap-like topic observed in target range).
- Validation on `359066951..359066952`:
  - before: `features_per_block=0.000`,
  - after: `features_per_block=0.500`, `feature_hit_ratio=0.143`.
- Added normalization profile via CLI (`--token0-decimals`, `--token1-decimals`).
- Added extraction quality counters:
  - per-receipt: `feature_extraction_stats`,
  - runtime aggregate: `recognized_logs`, `unknown_topic_logs`, `malformed_logs`.
- Added fixtures:
  - `tests/fixtures/receipt_swap_like_359066952.json`
  - `tests/fixtures/receipt_unknown_topics_359066951.json`
