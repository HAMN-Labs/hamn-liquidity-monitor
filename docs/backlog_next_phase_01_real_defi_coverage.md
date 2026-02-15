# Next Iteration Backlog: Phase 1 (Real DeFi Coverage)

## Scope
Increase detection signal on real Arbitrum DeFi activity ranges.

## Tasks
- [x] Add configurable `eth_getLogs` filters (topic0/address).
- [x] Add runtime coverage metrics (`logs_per_block`, `log_hit_ratio`, `features_per_block`, `feature_hit_ratio`).
- [x] Validate on known active block `359066951`.
- [ ] Select and document 3-5 high-activity replay ranges for DeFi validation.
- [ ] Add quick command presets for each selected range.
- [ ] Produce first coverage report from those ranges.

## Notes
- Start block default set to `359066951`.
- `log-topic0` filter confirmed non-zero on nearby active range (`1000001`).
