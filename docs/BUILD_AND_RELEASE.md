# Build and Release

## Release Build
Use optimized release profile:

```bash
cargo build --release
```

Binary path:
- `target/release/hamn_liquidity_monitor`

Configured release optimizations (`Cargo.toml`):
- `lto = "thin"`
- `codegen-units = 1`
- `strip = true`

## Quick Verification
Run offline validation in release mode:

```bash
cargo run --release -- --run-validation-set
```

Run short replay smoke:

```bash
export HAMN_RPC_URL="https://arb-mainnet.g.alchemy.com/v2/<your_key>"
cargo run --release -- \
  --start-block 359066951 \
  --end-block 359066952 \
  --receipt-limit 5 \
  --extract-features
```

## Preset Scripts
- Replay profile: `scripts/run_replay_profile.sh`
- Follow profile: `scripts/run_follow_profile.sh`

Both scripts require:
- `HAMN_RPC_URL` env variable.

Optional env overrides:
- `START_BLOCK`
- `END_BLOCK` (replay script)
- `POLL_INTERVAL_MS` (follow script)
- `RECEIPT_LIMIT`
- `QUEUE_CAPACITY`
