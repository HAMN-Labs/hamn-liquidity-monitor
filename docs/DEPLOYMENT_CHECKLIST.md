# Deployment Checklist

## 1. Host Preparation
- Create dedicated user (example: `hamn`).
- Create working directory (example: `/opt/hamn-liquidity-monitor`).
- Create writable state directory (example: `/var/lib/hamn`).
- Ensure outbound access to your RPC provider endpoint.

## 2. Build and Artifacts
- Build optimized binary:
  - `cargo build --release`
- Verify binary exists:
  - `target/release/hamn_liquidity_monitor`
- Run baseline validation:
  - `cargo run --release -- --run-validation-set`

## 3. Configuration
- Create env file (example: `/etc/hamn-liquidity-monitor.env`):
  - `HAMN_RPC_URL=https://arb-mainnet.g.alchemy.com/v2/<your_key>`
- Decide startup mode:
  - replay (fixed `--start-block/--end-block`)
  - follow (`--follow`)
- Decide observability mode:
  - `--log-format json --emit-metrics-json` for machine ingestion.

## 4. Service Installation (systemd)
- Copy unit file:
  - `deploy/systemd/hamn-liquidity-monitor.service`
- Adjust `User`, `Group`, `WorkingDirectory`, and `ExecStart` as needed.
- Install and enable:
  - `sudo cp deploy/systemd/hamn-liquidity-monitor.service /etc/systemd/system/`
  - `sudo systemctl daemon-reload`
  - `sudo systemctl enable --now hamn-liquidity-monitor`

## 5. Post-Deploy Verification
- Check service state:
  - `systemctl status hamn-liquidity-monitor`
- Check logs:
  - `journalctl -u hamn-liquidity-monitor -f`
- Confirm key runtime signals:
  - `preflight_ok`
  - `runtime_metrics`
  - `memory_metrics` (if memory enabled)
  - `sequence_metrics` (if sequences enabled)

## 6. Operational Safety
- Keep `--error-mode fail-soft` for unstable RPC environments.
- Enable periodic snapshots:
  - `--snapshot-interval-blocks <N>`
  - `--memory-snapshot-out /var/lib/hamn/memory_snapshot.json`
- Ensure snapshot path is writable by service user.
