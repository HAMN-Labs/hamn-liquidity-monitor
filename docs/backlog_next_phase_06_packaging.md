# Next Iteration Backlog: Phase 6 (Packaging)

## Scope
Prepare production-friendly packaging and deployment artifacts.

## Tasks
- [x] Document release build profile and optimization flags.
- [x] Add sample systemd unit file.
- [x] Add command preset scripts for replay and follow mode.
- [x] Add deployment checklist.

## Completion Notes
- Added release profile in `Cargo.toml` (`lto=thin`, `codegen-units=1`, `strip=true`).
- Added release docs in `docs/BUILD_AND_RELEASE.md`.
- Added systemd unit template: `deploy/systemd/hamn-liquidity-monitor.service`.
- Added preset scripts:
  - `scripts/run_replay_profile.sh`
  - `scripts/run_follow_profile.sh`
- Added deployment checklist: `docs/DEPLOYMENT_CHECKLIST.md`.
