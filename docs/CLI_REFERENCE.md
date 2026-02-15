# HAMN CLI Reference

## Usage
```bash
cargo run -- [OPTIONS]
```

## Core Options
- `--rpc-url <URL>`  
  RPC endpoint URL. Can also be provided via `HAMN_RPC_URL`.
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

## Sequence Modeling
- `--enable-sequences`  
  Enables transition map updates.
- `--sequence-smoothing-alpha <F64>` (default: `0.25`)  
  Laplace smoothing factor for transition probabilities.

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
