# HAMN CLI Reference

## Usage
```bash
cargo run -- [OPTIONS]
```

## Core Options
- `--rpc-url <URL>`  
  RPC endpoint URL. Can also be provided via `HAMN_RPC_URL`.
  Required for chain replay/follow modes; not required for `--run-validation-set`.
- `--start-block <U64>`  
  First block to process (default: `359066951`).
- `--end-block <U64>`  
  Last block to process. Required unless `--follow` is enabled.
- `--follow`  
  Polls chain head and continues processing new blocks.
- `--poll-interval-ms <U64>` (default: `2000`)  
  Poll interval in follow mode.

## Ingestion Options
- `--full-tx`  
  Request full transaction objects in `eth_getBlockByNumber`.
- `--fetch-logs`  
  Request `eth_getLogs` per processed block.
- `--log-topic0 <HEX>` (repeatable)  
  Optional topic0 filter(s) used with `--fetch-logs`.
- `--log-address <ADDRESS>` (repeatable)  
  Optional contract address filter(s) used with `--fetch-logs`.
- `--receipt-limit <USIZE>` (default: `0`)  
  Max receipts fetched per block.

## RPC Reliability
- `--rpc-timeout-ms <U64>` (default: `10000`)
- `--rpc-max-retries <U32>` (default: `3`)
- `--rpc-backoff-ms <U64>` (default: `250`)
- `--rpc-max-backoff-ms <U64>` (default: `2000`)

## Feature Extraction
- `--extract-features`  
  Enables feature extraction from receipt logs.
- `--features-out <PATH>`  
  Appends normalized feature records to NDJSON file for offline analysis.
- `--features-out-rotate-records <U64>` (default: `0`)  
  Rotates feature export file every N written records (`0` disables rotation).  
  In rotation mode files are written as `*.part000000.ndjson`, `*.part000001.ndjson`, etc.
- `--token0-decimals <U8>` (default: `18`)  
  Decimals used to scale token0 amounts during normalization.
- `--token1-decimals <U8>` (default: `18`)  
  Decimals used to scale token1 amounts during normalization.

## Sequence Modeling
- `--enable-sequences`  
  Enables transition map updates.
- `--sequence-smoothing-alpha <F64>` (default: `0.25`)  
  Laplace smoothing factor for transition probabilities.

## Alerting
- `--enable-alerts`  
  Enables rule-based online alert emission from normalized features.
- `--alert-min-volume-ln <F64>` (default: `1.0`)  
  Minimum normalized volume threshold for alert rule.
- `--alert-min-abs-imbalance <F64>` (default: `0.2`)  
  Minimum absolute imbalance threshold for alert rule.
- `--alert-min-gas-used <F64>` (default: `20000`)  
  Minimum transaction gas used threshold for alert rule.
- `--alert-min-gas-ln-swap-spike <F64>` (default: `10.8`)  
  Minimum normalized gas threshold for swap gas spike alerts.
- `--alert-cooldown-blocks <U64>` (default: `20`)  
  Minimum block distance between alerts for the same pool.

Severity model:
- `warning`: alert thresholds are met.
- `critical`: stronger signal (`volume` and `imbalance` >= 2x threshold, `gas_used` >= 1.5x threshold).

Alert rules:
- `high_imbalance_high_volume`
- `swap_gas_spike`

## Adaptive Memory
- `--enable-memory`  
  Enables adaptive memory match/new logic.
- `--memory-distance-threshold <F64>` (default: `0.35`)
- `--memory-decay-per-block <F64>` (default: `0.999`)
- `--memory-min-confidence <F64>` (default: `0.08`)
- `--memory-max-inactive-blocks <U64>` (default: `50000`)
- `--memory-min-occurrences-for-retention <U64>` (default: `2`)
- `--memory-noise-inactive-blocks <U64>` (default: `2000`)
- `--memory-max-patterns <USIZE>` (default: `50000`)
- `--memory-snapshot-in <PATH>`  
  Load memory snapshot at startup.
- `--memory-snapshot-out <PATH>`  
  Save memory snapshot on shutdown.

## Pipeline Runtime
- `--pipeline-queue-capacity <USIZE>` (default: `128`)  
  Bounded queue size between ingestion producer and processing consumer.
- `--topic0-top-n <USIZE>` (default: `10`)  
  Prints top-N observed `topic0` signatures from fetched logs.
- `--log-format <text|json>` (default: `text`)  
  Runtime event output format.
- `--emit-metrics-json`  
  Emits machine-readable JSON metric records (`type=metric`) for `runtime_metrics`, `memory_metrics`, `sequence_metrics`, and `validation_metrics`.
- `--skip-preflight`  
  Skips startup RPC preflight checks.
- `--heartbeat-interval-blocks <U64>` (default: `100`)  
  Emits follow-mode heartbeat every N processed blocks.
- `--snapshot-interval-blocks <U64>` (default: `0`)  
  Saves periodic memory snapshots every N blocks (`0` disables periodic checkpoints).
- `--error-mode <fail-soft|fail-fast>` (default: `fail-soft`)  
  Controls behavior on RPC ingestion errors.

## Validation Baseline
- `--run-validation-set`  
  Runs offline labeled validation and exits.
- `--validation-set-path <PATH>` (default: `tests/fixtures/validation_set.json`)  
  Path to validation set JSON.
- `--validation-min-precision <F64>` (default: `0.8`)  
  Minimum required proxy precision.
- `--validation-min-recall <F64>` (default: `0.8`)  
  Minimum required proxy recall.

## Example Presets
Replay with full pipeline:
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000200 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```

Follow mode with tighter RPC policy:
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1001000 \
  --follow \
  --poll-interval-ms 1000 \
  --rpc-timeout-ms 20000 \
  --rpc-max-retries 5 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```
