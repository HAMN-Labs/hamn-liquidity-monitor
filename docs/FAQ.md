# HAMN FAQ

## 1) Why do I get `features_extracted=0`?
Most often, the selected block range does not contain relevant DeFi liquidity events for current parsers.  
Try a more active range and increase `--receipt-limit`.

## 2) Which network is recommended first?
Arbitrum mainnet is a good starting point due to high activity and fast block flow.

## 3) How do I pass RPC URL safely?
Use environment variable:
```bash
export HAMN_RPC_URL="https://arb-mainnet.g.alchemy.com/v2/<your_key>"
```
Then run without embedding the key in command history.

## 4) What does `--pipeline-queue-capacity` affect?
It controls bounded queue size between ingestion and processing.  
Lower values increase backpressure; higher values smooth bursts but use more memory.

## 5) What should I tune first if throughput is low?
1. Increase `--pipeline-queue-capacity`.
2. Reduce `--receipt-limit`.
3. Increase RPC timeout/retries if provider is unstable.

## 6) What indicates healthy runtime?
In `runtime_metrics`:
- low `rpc_errors`,
- stable `throughput_rps`,
- manageable `avg_queue_backpressure_ms`,
- no growing `max_block_lag`.

## 7) Why is `p_swap_add` non-zero when no transitions exist?
Because smoothed probabilities use Laplace smoothing (`--sequence-smoothing-alpha`).  
For raw value, check `p_swap_add_raw`.

## 8) How do memory snapshots work?
- Load at startup: `--memory-snapshot-in <file>`
- Save at shutdown: `--memory-snapshot-out <file>`
Use both to persist adaptive memory across runs.

## 9) How to reduce memory churn?
- Increase `--memory-min-confidence`
- Reduce `--memory-max-inactive-blocks`
- Review `--memory-distance-threshold` to avoid excessive pattern fragmentation

## 10) What is the minimal command for full pipeline?
```bash
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --extract-features \
  --enable-memory \
  --enable-sequences
```

## 11) Can I run in near-real-time mode?
Yes, with `--follow` and a poll interval:
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

## 12) Where should I start troubleshooting?
1. `docs/USER_GUIDE.md`
2. `docs/CLI_REFERENCE.md`
3. `docs/OPERATIONS_RUNBOOK.md`
4. `docs/EXAMPLES.md`

## 13) Can I filter logs by event signature or contract?
Yes. Use:
- `--log-topic0 <event_topic_hash>` (repeatable)
- `--log-address <contract_address>` (repeatable)
Together with `--fetch-logs`.
