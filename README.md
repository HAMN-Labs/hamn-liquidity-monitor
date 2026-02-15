# HAMN: Hierarchical Adaptive Memory Network for Liquidity Analysis

**HAMN** is an intelligent system designed to analyze and forecast liquidity flows within blockchain ecosystems such as Ethereum and Arbitrum. The system operates in a streaming mode and autonomously adapts to market shifts without the need for manual rule updates.

## Key Features
*   **Real-Time Liquidity Detection:** The system identifies token swaps, liquidity additions and removals, arbitrage sequences, and capital migrations.
*   **ABI-Agnostic Recognition:** HAMN recognizes new pools and protocols based on behavioral features—such as token movements, reserve changes, and Mint/Burn operations—rather than relying on static contract addresses or ABIs.
*   **Predictive Analytics:** By constructing probabilistic **transition maps**, the system forecasts likely subsequent actions of market participants based on accumulated pattern memory.
*   **Bot and Whale Monitoring:** The architecture tracks MEV bot strategies and large-scale participant behavior by analyzing transaction complexity and specific asset movement patterns.

## Technical Architecture
The HAMN architecture implements a full real-time data processing cycle:
1.  **Data Ingestion:** Fetching blocks, transactions, and event logs from the network.
2.  **Feature Extraction:** Calculating behavioral feature vectors, including reserve shifts and transaction complexity.
3.  **Adaptive Memory:** Matching vectors against existing patterns, creating new entries, and automatically pruning obsolete data to ensure stability.
4.  **Sequence Mapping:** Establishing action sequences and calculating transition probabilities for the predictive model.
5.  **Online Processing:** High-throughput streaming with low latency, optimized for fast-block networks like **Arbitrum**.

## Implementation Roadmap (MVP)
The development is structured into six key stages:
*   **Stage 1:** Implementing block and transaction fetching mechanisms.
*   **Stage 2:** Building the behavioral feature extraction engine.
*   **Stage 3:** Developing the pattern memory and vector matching logic.
*   **Stage 4:** Integrating memory stabilization and cleanup algorithms.
*   **Stage 5:** Training the sequence model and constructing the transition map.
*   **Stage 6:** Launching full-scale online detection of liquidity events.

## Success Metrics
The MVP is considered successful upon meeting the following criteria:
*   Correct detection of swaps and liquidity changes.
*   Stable memory performance over extended periods.
*   Accurate discovery of new pools.

## Use Cases
*   Monitoring DeFi activity and pool liquidity fluctuations.
*   Automatically discovering new protocols and pools immediately upon deployment.
*   Generating trading signals based on probabilistic capital movement forecasts.
*   Researching market patterns and MEV bot behaviors.

---
*This project is implemented in accordance with technical specification version 0.2.*

## Local Run (Stage 1, Rust ingestion)

```bash
export HAMN_RPC_URL="https://arb-mainnet.g.alchemy.com/v2/<your_alchemy_key>"
cargo run -- --start-block 1 --end-block 1
```

```bash
# Logs + receipts sample
cargo run -- \
  --start-block 2 \
  --end-block 3 \
  --fetch-logs \
  --receipt-limit 1
```

```bash
# Logs with topic filter (example: UniswapV2 Swap topic0)
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --fetch-logs \
  --topic0-top-n 10 \
  --log-topic0 0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822
```

```bash
# Feature extraction from receipts
cargo run -- \
  --start-block 2 \
  --end-block 10 \
  --receipt-limit 5 \
  --extract-features \
  --token0-decimals 18 \
  --token1-decimals 18
```

```bash
# Near-real-time polling (bounded by end block)
cargo run -- \
  --start-block 2 \
  --end-block 10 \
  --follow \
  --poll-interval-ms 1000
```

```bash
# Adaptive memory (load/save snapshot)
cargo run -- \
  --start-block 1000000 \
  --end-block 1000010 \
  --receipt-limit 20 \
  --enable-memory \
  --memory-distance-threshold 0.35 \
  --memory-snapshot-out memory_snapshot.json
```

```bash
# Adaptive memory stabilization tuning
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --enable-memory \
  --memory-decay-per-block 0.999 \
  --memory-min-confidence 0.08 \
  --memory-max-inactive-blocks 50000 \
  --memory-noise-inactive-blocks 2000 \
  --memory-min-occurrences-for-retention 2 \
  --memory-max-patterns 50000
```

```bash
# Sequence mapping + transition probabilities
cargo run -- \
  --start-block 1000000 \
  --end-block 1000100 \
  --receipt-limit 20 \
  --extract-features \
  --enable-sequences \
  --sequence-smoothing-alpha 0.25
```

```bash
# Stage 6 online runtime (bounded pipeline + runtime metrics)
cargo run -- \
  --start-block 2 \
  --end-block 20 \
  --receipt-limit 2 \
  --extract-features \
  --enable-memory \
  --enable-sequences \
  --pipeline-queue-capacity 4
```

```bash
# Runtime hardening controls (heartbeat/snapshots/error mode)
cargo run -- \
  --rpc-url "$HAMN_RPC_URL" \
  --end-block 359066952 \
  --follow \
  --fetch-logs \
  --enable-memory \
  --heartbeat-interval-blocks 10 \
  --snapshot-interval-blocks 100 \
  --memory-snapshot-out memory_snapshot.json \
  --error-mode fail-soft
```

## Documentation
- User guide (English): `docs/USER_GUIDE.md`
- CLI reference (English): `docs/CLI_REFERENCE.md`
- Operations runbook (English): `docs/OPERATIONS_RUNBOOK.md`
- Example scenarios (English): `docs/EXAMPLES.md`
- FAQ (English): `docs/FAQ.md`
- Next iteration plan: `docs/NEXT_ITERATION_PLAN.md`
- Next iteration backlogs:
  - `docs/backlog_next_phase_01_real_defi_coverage.md`
  - `docs/backlog_next_phase_02_feature_quality.md`
  - `docs/backlog_next_phase_03_detection_baseline.md`
  - `docs/backlog_next_phase_04_runtime_hardening.md`
  - `docs/backlog_next_phase_05_observability_ops.md`
  - `docs/backlog_next_phase_06_packaging.md`
- Phase 1 coverage report: `docs/PHASE1_COVERAGE_REPORT.md`
- Internal execution workflow: `docs/workflow.md`
