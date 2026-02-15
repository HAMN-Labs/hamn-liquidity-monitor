# Next Iteration Backlog: Phase 5 (Observability and Operations)

## Scope
Improve machine-readable observability and operational ergonomics.

## Tasks
- [x] Add JSON log output mode.
- [x] Add machine-readable metrics output stream.
- [x] Add startup preflight checks and status output.
- [x] Expand runbook with tuning tables and incident playbooks.

## Completion Notes
- Added `--log-format text|json` and unified event emitter.
- Added `--emit-metrics-json` for machine-readable metric lines (`type=metric` JSON objects).
- Added startup preflight with `preflight_started/preflight_ok/preflight_warning` and bypass via `--skip-preflight`.
- Updated operations docs with tuning guidance and incident response playbooks.
