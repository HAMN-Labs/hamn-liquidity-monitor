# Phase 1 Coverage Report

Date: 2026-02-15  
Network: Arbitrum mainnet (Alchemy RPC)

## Method
Command template:
```bash
target/debug/hamn_liquidity_monitor \
  --rpc-url "$HAMN_RPC_URL" \
  --start-block <START> \
  --end-block <END> \
  --fetch-logs \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences \
  --pipeline-queue-capacity 32
```

## Selected Ranges and Results
1. `1000000..1000010`
  - blocks: `11`
  - logs_per_block: `8.000`
  - log_hit_ratio: `1.000`
  - features_per_block: `1.636`
  - feature_hit_ratio: `1.636`
  - rpc_errors: `6`

2. `359066951..359066960`
  - blocks: `10`
  - logs_per_block: `10.200`
  - log_hit_ratio: `1.000`
  - features_per_block: `0.000`
  - feature_hit_ratio: `0.000`
  - rpc_errors: `0`

3. `359067000..359067010`
  - blocks: `11`
  - logs_per_block: `6.727`
  - log_hit_ratio: `1.000`
  - features_per_block: `0.000`
  - feature_hit_ratio: `0.000`
  - rpc_errors: `0`

4. `359070000..359070010`
  - blocks: `11`
  - logs_per_block: `22.455`
  - log_hit_ratio: `1.000`
  - features_per_block: `0.000`
  - feature_hit_ratio: `0.000`
  - rpc_errors: `0`

5. `359100000..359100010`
  - blocks: `11`
  - logs_per_block: `8.182`
  - log_hit_ratio: `0.909`
  - features_per_block: `0.000`
  - feature_hit_ratio: `0.000`
  - rpc_errors: `0`

## Summary
- Best feature-producing range in this sample: `1000000..1000010`.
- High log density but zero features in modern ranges indicates parser coverage gap for current event signatures.
- Next action: Phase 2 feature parser extension for protocol/event variants seen in ranges around `359066951+`.
