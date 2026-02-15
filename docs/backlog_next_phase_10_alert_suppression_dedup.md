# Next Iteration Backlog: Phase 10 (Alert Suppression and Dedup)

## Scope
Reduce alert noise in production by adding suppression windows and dedup behavior.

## Tasks
- [x] Add maintenance suppression window by block range.
- [x] Add per-transaction alert dedup.
- [x] Add suppression/dedup counters in runtime metrics.
- [x] Add config presets for maintenance events.
- [x] Add periodic report for suppressed/deduped alerts.

## Completion Notes
- Added maintenance flags:
  - `--alert-maintenance-start-block`
  - `--alert-maintenance-end-block`
- Added dedup flag:
  - `--alert-dedupe-by-tx`
- Added runtime counters:
  - `alerts_suppressed_maintenance`
  - `alerts_deduped_tx`
- Added periodic report flag:
  - `--alert-report-interval-blocks`
- Added maintenance preset script:
  - `scripts/run_follow_maintenance_profile.sh`
