# Next Iteration Backlog: Phase 3 (Detection Quality Baseline)

## Scope
Establish measurable quality baseline for event detection.

## Tasks
- [x] Define proxy metrics for swap/add/remove detection quality.
- [x] Build a small labeled validation set.
- [x] Add validation runner command.
- [x] Set acceptance thresholds and document pass/fail criteria.

## Notes
- Added validation set file: `tests/fixtures/validation_set.json`.
- Added runner: `--run-validation-set`.
- Added thresholds:
  - `--validation-min-precision` (default `0.8`)
  - `--validation-min-recall` (default `0.8`)
- Current baseline result:
  - `precision_proxy=1.0000`
  - `recall_proxy=1.0000`
