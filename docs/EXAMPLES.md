# HAMN Examples

## 1. Minimal Connectivity Check
Purpose: verify RPC access and basic block ingestion.

```bash
cargo run -- \
  --start-block 1 \
  --end-block 1
```

Expected signals:
- `block=...`
- no RPC errors

## 2. Replay with Receipts
Purpose: validate receipt ingestion on a fixed range.

```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000020 \
  --receipt-limit 10
```

Expected signals:
- `receipt tx=...`
- `receipts_fetched=...` per block

## 3. Feature Extraction Replay
Purpose: ensure feature extraction path is active.

```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --extract-features
```

Expected signals:
- `features_extracted=...`
- `feature event=...` when matching logs exist

## 4. Memory + Sequence Pipeline
Purpose: run full model components on replay.

```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000200 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```

Expected signals:
- `memory_metrics ...`
- `sequence_metrics ...`

## 5. Follow Mode (Near Real-Time)
Purpose: continuous processing with polling.

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

Expected signals:
- continuous `block=...`
- `runtime_metrics ...` on shutdown

## 6. Backpressure Stress Profile
Purpose: validate bounded queue behavior under tighter capacity.

```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000200 \
  --receipt-limit 30 \
  --extract-features \
  --enable-memory \
  --enable-sequences \
  --pipeline-queue-capacity 2
```

Expected signals:
- `runtime_metrics ...`
- non-zero `avg_queue_backpressure_ms` possible under load

## 7. Snapshot Load/Save Cycle
Purpose: validate memory persistence between runs.

First run (save):
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --memory-snapshot-out memory_snapshot.json
```

Second run (load):
```bash
cargo run -- \
  --start-block 1000101 \
  --end-block 1000200 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --memory-snapshot-in memory_snapshot.json \
  --memory-snapshot-out memory_snapshot.json
```

Expected signals:
- `memory_snapshot_saved path=... patterns=...`

## 8. RPC Stability Tuning
Purpose: improve behavior under intermittent RPC latency/errors.

```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000200 \
  --receipt-limit 20 \
  --rpc-timeout-ms 20000 \
  --rpc-max-retries 5 \
  --rpc-backoff-ms 300 \
  --rpc-max-backoff-ms 5000
```

Expected signals:
- reduced transient fetch failures
- stable `runtime_metrics rpc_errors=...`

## 9. DeFi Coverage with Topic Filter
Purpose: focus log ingestion on known liquidity-related event topics.

```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000200 \
  --fetch-logs \
  --log-topic0 0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822
```

Expected signals:
- `logs_range=... total_logs=... topic0_filters=1`

## 10. Phase 1 Coverage Presets
Purpose: replay selected ranges for coverage comparison.

Preset A:
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000010 \
  --fetch-logs \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```

Preset B:
```bash
cargo run -- \
  --start-block 359066951 \
  --end-block 359066960 \
  --fetch-logs \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```

Preset C:
```bash
cargo run -- \
  --start-block 359070000 \
  --end-block 359070010 \
  --fetch-logs \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```

## 11. Validation Baseline Runner
Purpose: run labeled offline baseline and enforce quality thresholds.

```bash
cargo run -- \
  --run-validation-set \
  --validation-set-path tests/fixtures/validation_set.json \
  --validation-min-precision 0.8 \
  --validation-min-recall 0.8
```

Expected signals:
- `validation_metrics ...`
- `validation_status=pass ...`

## 12. Runtime Hardening Profile
Purpose: verify heartbeat, periodic snapshots, and fail-soft policy.

```bash
cargo run -- \
  --rpc-url "$HAMN_RPC_URL" \
  --end-block 359066952 \
  --follow \
  --fetch-logs \
  --receipt-limit 2 \
  --enable-memory \
  --extract-features \
  --heartbeat-interval-blocks 1 \
  --snapshot-interval-blocks 1 \
  --memory-snapshot-out memory_snapshot_phase4.json \
  --error-mode fail-soft
```

Expected signals:
- `heartbeat block=...`
- `memory_snapshot_saved mode=periodic ...`
- final `runtime_metrics ...`

## 13. JSON Observability Profile
Purpose: emit structured events and machine-readable metrics.

```bash
cargo run -- \
  --rpc-url "$HAMN_RPC_URL" \
  --start-block 359066951 \
  --end-block 359066952 \
  --fetch-logs \
  --receipt-limit 5 \
  --extract-features \
  --enable-memory \
  --enable-sequences \
  --log-format json \
  --emit-metrics-json
```

Expected signals:
- JSON event records (`{"type":"event","event":"block",...}`)
- JSON metric records (`{"type":"metric","metric":"runtime_metrics",...}`)
- startup preflight records (`preflight_started`, `preflight_ok`)

## 14. Feature Export Profile
Purpose: persist extracted normalized features for offline labeling and analysis.

```bash
cargo run -- \
  --rpc-url "$HAMN_RPC_URL" \
  --start-block 359066951 \
  --end-block 359066960 \
  --receipt-limit 20 \
  --extract-features \
  --features-out data/features.ndjson
```

Expected signals:
- `features_output_enabled ...`
- `feature ...`
- NDJSON records in `data/features.ndjson`

## 15. Feature Export with Rotation
Purpose: keep file sizes bounded during long replay/follow runs.

```bash
cargo run -- \
  --rpc-url "$HAMN_RPC_URL" \
  --start-block 359066951 \
  --end-block 359067200 \
  --receipt-limit 20 \
  --extract-features \
  --features-out data/features.ndjson \
  --features-out-rotate-records 50000
```

Expected signals:
- `features_output_enabled ... rotate_records=50000`
- `features_output_rotated ...`
- files like `data/features.part000000.ndjson`, `data/features.part000001.ndjson`

## 16. Export Dataset Quick Stats
Purpose: inspect class mix and top pools in exported NDJSON.

```bash
scripts/features_stats.sh data/features.ndjson
```

Expected signals:
- `total_lines=...`
- `event_swap=...`, `event_add_liquidity=...`, `event_remove_liquidity=...`
- `top_pool count=... pool=...`

## 17. Online Alerting Profile
Purpose: emit operational alerts for high-imbalance/high-volume liquidity events.

```bash
cargo run -- \
  --rpc-url "$HAMN_RPC_URL" \
  --start-block 359066951 \
  --end-block 359067200 \
  --receipt-limit 20 \
  --enable-alerts \
  --alert-enable-high-imbalance-high-volume true \
  --alert-enable-swap-gas-spike true \
  --alert-enable-burst-window true \
  --alert-dedupe-by-tx true \
  --alert-min-volume-ln 1.0 \
  --alert-min-abs-imbalance 0.2 \
  --alert-min-gas-used 20000 \
  --alert-min-gas-ln-swap-spike 10.8 \
  --alert-burst-window-blocks 50 \
  --alert-burst-min-events 3 \
  --alert-cooldown-blocks 20 \
  --alert-report-interval-blocks 100 \
  --alert-maintenance-start-block 0 \
  --alert-maintenance-end-block 0 \
  --log-format json \
  --emit-metrics-json
```

Expected signals:
- `alert kind=high_imbalance_high_volume|swap_gas_spike|burst_window ...` events
- `{"type":"metric","metric":"alerts",...}` lines
- `alert_noise_report ...` every 100 blocks
- `runtime_metrics ... alerts_emitted=... alerts_suppressed_maintenance=... alerts_deduped_tx=...`
