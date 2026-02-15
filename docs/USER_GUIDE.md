# HAMN User Guide

## Overview
HAMN is a Rust-based liquidity monitoring pipeline for EVM chains.  
Current implementation supports:
- block/receipt/log ingestion via JSON-RPC,
- liquidity feature extraction from receipt logs,
- adaptive memory (pattern match/new + stabilization),
- sequence transition modeling with smoothing,
- bounded online runtime pipeline with operational metrics.

Related docs:
- CLI flags and presets: `docs/CLI_REFERENCE.md`
- Operational procedures: `docs/OPERATIONS_RUNBOOK.md`
- Ready-to-run scenarios: `docs/EXAMPLES.md`
- Frequently asked questions: `docs/FAQ.md`

## Prerequisites
- Rust toolchain installed (`cargo`, `rustc`).
- Access to an EVM RPC endpoint (for example Alchemy Arbitrum mainnet URL).

## Setup
1. Set RPC URL:
```bash
export HAMN_RPC_URL="https://arb-mainnet.g.alchemy.com/v2/<your_alchemy_key>"
```
2. Build and run:
```bash
cargo run -- --start-block 1 --end-block 1
```

## Quick Start Commands
Basic replay:
```bash
cargo run -- --start-block 1000000 --end-block 1000010 --receipt-limit 10
```

Enable feature extraction:
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --extract-features
```

Enable adaptive memory:
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --enable-memory \
  --memory-distance-threshold 0.35 \
  --memory-snapshot-out memory_snapshot.json
```

Enable sequence modeling:
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --extract-features \
  --enable-sequences \
  --sequence-smoothing-alpha 0.25
```

Full online runtime (bounded queue + metrics):
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000200 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences \
  --pipeline-queue-capacity 64
```

Follow mode:
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1001000 \
  --follow \
  --poll-interval-ms 1500 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```

Log filtering (topic0 / address):
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --fetch-logs \
  --log-topic0 0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822
```

## Main CLI Options
General:
- `--start-block` (default: `359066951`), `--end-block`
- `--follow`, `--poll-interval-ms`
- `--full-tx`, `--fetch-logs`, `--receipt-limit`

RPC reliability:
- `--rpc-timeout-ms`
- `--rpc-max-retries`
- `--rpc-backoff-ms`
- `--rpc-max-backoff-ms`

Pipeline control:
- `--pipeline-queue-capacity`
- `--topic0-top-n`

Feature extraction:
- `--extract-features`

Adaptive memory:
- `--enable-memory`
- `--memory-distance-threshold`
- `--memory-decay-per-block`
- `--memory-min-confidence`
- `--memory-max-inactive-blocks`
- `--memory-min-occurrences-for-retention`
- `--memory-noise-inactive-blocks`
- `--memory-max-patterns`
- `--memory-snapshot-in`
- `--memory-snapshot-out`

Sequence model:
- `--enable-sequences`
- `--sequence-smoothing-alpha`

## Runtime Output
Important log lines:
- `block=...` / `receipt tx=...` / `features_extracted=...`
- `memory_metrics ...`
- `sequence_metrics ...`
- `runtime_metrics ...`
- `topic0_top rank=... count=... topic=...`

`runtime_metrics` includes:
- throughput (`throughput_rps`),
- fetch/processing latency averages,
- queue backpressure and queue latency,
- RPC error count,
- max observed block lag.

## Troubleshooting
No features extracted:
- Use a block range with real DeFi activity (early chain blocks often have zero useful logs).
- Increase `--receipt-limit`.
- Enable `--fetch-logs` to verify chain activity in selected range.

High RPC errors:
- Increase timeout/retries.
- Example: `--rpc-timeout-ms 20000 --rpc-max-retries 5`.

Low throughput:
- Increase `--pipeline-queue-capacity`.
- Decrease `--receipt-limit` if RPC is the bottleneck.

Memory grows too fast:
- Lower `--memory-max-patterns`.
- Increase pruning aggressiveness:
  - higher `--memory-min-confidence`,
  - lower `--memory-max-inactive-blocks`.

## Recommended First Validation
1. Run replay on a DeFi-active range.
2. Confirm non-zero `features_extracted`.
3. Confirm `memory_metrics.active_patterns > 0`.
4. Confirm `sequence_metrics.total_transitions > 0`.
5. Save memory snapshot and restart using `--memory-snapshot-in`.
