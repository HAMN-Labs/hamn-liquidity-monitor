# Next Iteration Backlog: Phase 9 (Multi-Rule Alerting)

## Scope
Evolve online alerting from a single rule to a maintainable multi-rule set.

## Tasks
- [x] Add second alert rule (`swap_gas_spike`) with dedicated threshold.
- [x] Track per-rule counters in runtime metrics.
- [x] Ensure cooldown is applied per `rule+pool` instead of per pool only.
- [ ] Add window-based burst rule (N alerts in M blocks).
- [ ] Add optional per-rule enable/disable flags.

## Completion Notes
- Added CLI threshold `--alert-min-gas-ln-swap-spike`.
- Added per-rule metrics:
  - `alerts_rule_high_imbalance_high_volume`
  - `alerts_rule_swap_gas_spike`
- Cooldown key now uses `kind:pool`.
