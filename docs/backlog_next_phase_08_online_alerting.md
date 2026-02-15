# Next Iteration Backlog: Phase 8 (Online Alerting)

## Scope
Add practical online alerting on top of extracted features for operations and monitoring.

## Tasks
- [x] Add rule-based alert emission in runtime.
- [x] Add cooldown mechanism to reduce repeated alerts for the same pool.
- [x] Expose alert thresholds via CLI options.
- [ ] Add alert severity levels and per-rule counters.
- [ ] Add alert runbook section with response playbooks.

## Completion Notes
- Added `--enable-alerts` and threshold flags:
  - `--alert-min-volume-ln`
  - `--alert-min-abs-imbalance`
  - `--alert-min-gas-used`
  - `--alert-cooldown-blocks`
- Added `alert` runtime event and `alerts` metric stream.
- Added `alerts_emitted` counter in `runtime_metrics`.
