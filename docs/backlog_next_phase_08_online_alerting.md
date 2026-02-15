# Next Iteration Backlog: Phase 8 (Online Alerting)

## Scope
Add practical online alerting on top of extracted features for operations and monitoring.

## Tasks
- [x] Add rule-based alert emission in runtime.
- [x] Add cooldown mechanism to reduce repeated alerts for the same pool.
- [x] Expose alert thresholds via CLI options.
- [x] Add alert severity levels and per-rule counters.
- [x] Add alert runbook section with response playbooks.

## Completion Notes
- Added `--enable-alerts` and threshold flags:
  - `--alert-min-volume-ln`
  - `--alert-min-abs-imbalance`
  - `--alert-min-gas-used`
  - `--alert-cooldown-blocks`
- Added `alert` runtime event and `alerts` metric stream.
- Added `alerts_emitted` counter in `runtime_metrics`.
- Added alert severity levels (`warning`, `critical`).
- Added counters: `alerts_warning`, `alerts_critical`, `alerts_rule_high_imbalance_high_volume`.
- Added runbook response playbooks in `docs/OPERATIONS_RUNBOOK.md`.
