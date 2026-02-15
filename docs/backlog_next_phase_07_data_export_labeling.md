# Next Iteration Backlog: Phase 7 (Data Export and Labeling)

## Scope
Prepare high-quality offline datasets for detection improvement and future model training.

## Tasks
- [x] Add runtime feature export to NDJSON file.
- [x] Add optional export rotation strategy for long follow runs.
- [x] Add lightweight schema doc for exported records.
- [x] Add labeling workflow template and quality rules.
- [x] Add quick stats command/script for exported datasets.

## Completion Notes
- Added `--features-out <PATH>` flag.
- Runtime now appends normalized feature records to NDJSON output for offline analysis.
- Added `--features-out-rotate-records <N>` and `features_output_rotated` event.
- Rotation naming: `*.part000000.ndjson`, `*.part000001.ndjson`, ...
- Added schema reference: `docs/FEATURE_EXPORT_SCHEMA.md`.
- Added labeling workflow: `docs/LABELING_WORKFLOW.md`.
- Added dataset quick-stats script: `scripts/features_stats.sh`.
