# HAMN Operations Runbook

## Startup Checklist
1. Confirm `HAMN_RPC_URL` is set and valid.
2. Run preflight (default behavior, do not pass `--skip-preflight` unless intentionally bypassing checks).
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
6. Decide observability format:
   - text logs (`--log-format text`)
   - structured logs (`--log-format json --emit-metrics-json`)

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
  - `alerts_emitted`
  - `alerts_warning`
  - `alerts_critical`
  - `alerts_rule_high_imbalance_high_volume`
  - `alerts_rule_swap_gas_spike`
  - `alerts_rule_burst_window`
- `memory_metrics`:
  - `active_patterns`
  - `match_ratio`
  - `churn_rate`
- `sequence_metrics`:
  - `total_transitions`
  - `unique_transitions`
- preflight:
  - `preflight_started`
  - `preflight_ok`
  - `preflight_warning`
- alerting:
  - `alert kind=high_imbalance_high_volume|swap_gas_spike|burst_window severity=warning|critical ...`
  - `metric=alerts`

## Tuning Table
- Goal: reduce RPC failures
  Action: increase `--rpc-timeout-ms`, `--rpc-max-retries`, `--rpc-max-backoff-ms`
  Watch: `rpc_errors`
- Goal: reduce pipeline lag
  Action: increase `--pipeline-queue-capacity`, reduce `--receipt-limit`
  Watch: `max_block_lag`, `avg_queue_backpressure_ms`
- Goal: reduce memory noise
  Action: increase `--memory-min-confidence`, reduce `--memory-max-inactive-blocks`
  Watch: `memory_metrics.churn_rate`
- Goal: monitoring integration
  Action: use `--log-format json --emit-metrics-json`
  Watch: `type=metric` stream ingestion in your collector
- Goal: reduce alert noise
  Action: increase `--alert-min-volume-ln`, `--alert-min-abs-imbalance`, `--alert-min-gas-used`, `--alert-cooldown-blocks`
  Watch: `alerts_emitted`, `alerts_warning`
- Goal: catch only extreme events
  Action: keep default thresholds and treat `severity=critical` as pager signal
  Watch: `alerts_critical`

## Incident Response
### High RPC Errors
- Increase:
  - `--rpc-timeout-ms`
  - `--rpc-max-retries`
- Verify provider rate limits and API key status.
- If failures must halt the process, use `--error-mode fail-fast`.

### Persistent Block Lag
- Increase `--pipeline-queue-capacity`.
- Reduce `--receipt-limit`.
- Raise poll interval in follow mode.
- Verify queue pressure through `avg_queue_backpressure_ms`.

### No Features Extracted
- Move to a DeFi-heavy block range.
- Enable `--fetch-logs` to confirm on-chain activity.
- Increase `--receipt-limit`.

### Memory Churn Too High
- Increase `--memory-min-confidence`.
- Decrease `--memory-max-inactive-blocks`.
- Increase `--memory-distance-threshold` only if over-fragmentation is observed.

### Invalid Startup Range
- Inspect preflight output (`preflight_warning` or preflight failure message).
- Fix `--start-block/--end-block` relative to current chain tip.
- Use `--skip-preflight` only for intentional dry-run behavior.

### Alert Spike (Warning)
- Validate chain context: check if spike aligns with known volatile market period.
- Temporarily raise:
  - `--alert-min-volume-ln`
  - `--alert-min-abs-imbalance`
- Increase `--alert-cooldown-blocks` for repeated pool noise.

### Critical Alert Triggered
- Inspect event payload (`tx`, `pool`, `volume_ln`, `abs_imbalance`, `gas_used`).
- Check neighboring blocks for repeated signals from same pool.
- If repeated critical alerts persist, switch runtime to focused mode:
  - enable `--fetch-logs`
  - apply `--log-address` filter for impacted pool/protocol contracts.
- Record incident with timestamp, block, tx, pool, and action taken.

### Burst Window Alert Triggered
- Treat as escalation signal for repeated alerts in short horizon.
- Check recent sequence of alerts for the same pool and correlated market events.
- Consider temporarily raising:
  - `--alert-burst-min-events`
  - `--alert-burst-window-blocks`
- If burst is expected (known event), keep configuration and annotate incident as expected volatility.

## Shutdown
1. Stop process gracefully.
2. Ensure snapshot is saved (`--memory-snapshot-out`).
3. Record latest `runtime_metrics` line for trend tracking.
