# Labeling Workflow

## Goal
Create a small high-quality labeled set for iterative parser/detector improvements.

## Steps
1. Export features from a DeFi-active range using `--features-out`.
2. Sample candidate transactions from NDJSON (balanced by `event` when possible).
3. For each sampled transaction, inspect chain evidence (logs/receipt) and assign label:
   - `swap`
   - `add_liquidity`
   - `remove_liquidity`
   - `none/uncertain`
4. Store labeled rows in a review file (CSV/JSON) with:
   - `tx`
   - `block`
   - `predicted_event`
   - `label_event`
   - `reviewer`
   - `notes`
5. Reject ambiguous samples from acceptance metrics until reviewed by 2nd reviewer.
6. Convert accepted labels into fixtures for validation set updates.

## Quality Rules
- Use consistent event definitions across reviewers.
- Keep `none/uncertain` explicit; do not force-fit labels.
- Require at least one evidence note per corrected label.
- Track per-reviewer disagreement rate.
