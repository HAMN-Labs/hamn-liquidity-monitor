# Next Iteration Backlog: Phase 9 (Multi-Rule Alerting)

## Scope
Evolve online alerting from a single rule to a maintainable multi-rule set.

## Tasks
- [x] Add second alert rule (`swap_gas_spike`) with dedicated threshold.
- [x] Track per-rule counters in runtime metrics.
- [x] Ensure cooldown is applied per `rule+pool` instead of per pool only.
- [x] Add window-based burst rule (N alerts in M blocks).
- [x] Add optional per-rule enable/disable flags.

## Completion Notes
- Added CLI threshold `--alert-min-gas-ln-swap-spike`.
- Added per-rule metrics:
  - `alerts_rule_high_imbalance_high_volume`
  - `alerts_rule_swap_gas_spike`
- Added per-rule metric:
  - `alerts_rule_burst_window`
- Cooldown key now uses `kind:pool`.
- Added per-rule toggles:
  - `--alert-enable-high-imbalance-high-volume`
  - `--alert-enable-swap-gas-spike`
  - `--alert-enable-burst-window`
- Added burst-window controls:
  - `--alert-burst-window-blocks`
  - `--alert-burst-min-events`
