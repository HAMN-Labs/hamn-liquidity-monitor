# HAMN Operations Runbook

## Startup Checklist
1. Confirm `HAMN_RPC_URL` is set and valid.
2. Choose a block range with expected DeFi activity.
3. Decide runtime mode:
   - replay (`--start-block`, `--end-block`)
   - follow mode (`--follow`)
4. Set processing profile:
   - `--receipt-limit`
   - `--pipeline-queue-capacity`
5. Enable components as needed:
   - `--extract-features`
   - `--enable-memory`
   - `--enable-sequences`

## Recommended Production-Like Command
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1005000 \
  --follow \
  --poll-interval-ms 1500 \
  --receipt-limit 30 \
  --extract-features \
  --enable-memory \
  --enable-sequences \
  --pipeline-queue-capacity 128 \
  --memory-snapshot-out memory_snapshot.json
```

## Key Runtime Signals
- `runtime_metrics`:
  - `throughput_rps`
  - `avg_queue_backpressure_ms`
  - `avg_queue_latency_ms`
  - `rpc_errors`
  - `max_block_lag`
- `memory_metrics`:
  - `active_patterns`
  - `match_ratio`
  - `churn_rate`
- `sequence_metrics`:
  - `total_transitions`
  - `unique_transitions`

## Incident Response
### High RPC Errors
- Increase:
  - `--rpc-timeout-ms`
  - `--rpc-max-retries`
- Verify provider rate limits and API key status.

### Persistent Block Lag
- Increase `--pipeline-queue-capacity`.
- Reduce `--receipt-limit`.
- Raise poll interval in follow mode.

### No Features Extracted
- Move to a DeFi-heavy block range.
- Enable `--fetch-logs` to confirm on-chain activity.
- Increase `--receipt-limit`.

### Memory Churn Too High
- Increase `--memory-min-confidence`.
- Decrease `--memory-max-inactive-blocks`.
- Increase `--memory-distance-threshold` only if over-fragmentation is observed.

## Shutdown
1. Stop process gracefully.
2. Ensure snapshot is saved (`--memory-snapshot-out`).
3. Record latest `runtime_metrics` line for trend tracking.
