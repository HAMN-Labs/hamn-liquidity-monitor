# Next Iteration Backlog: Phase 4 (Runtime Hardening)

## Scope
Improve long-running stability and graceful operations.

## Tasks
- [x] Add periodic heartbeat status line in follow mode.
- [x] Add graceful shutdown signal handling and final flush.
- [x] Add optional periodic memory snapshot checkpoints.
- [x] Add fail-fast / fail-soft runtime error mode.

## Notes
- Added heartbeat config: `--heartbeat-interval-blocks`.
- Added graceful shutdown via `Ctrl+C` signal listener with producer stop + final flush.
- Added periodic snapshot checkpoints:
  - `--snapshot-interval-blocks`
  - uses `--memory-snapshot-out` path.
- Added runtime error policy:
  - `--error-mode fail-soft|fail-fast`.
