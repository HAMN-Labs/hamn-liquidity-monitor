#!/usr/bin/env bash
set -euo pipefail

: "${HAMN_RPC_URL:?HAMN_RPC_URL is required}"

START_BLOCK="${START_BLOCK:-359066951}"
POLL_INTERVAL_MS="${POLL_INTERVAL_MS:-1500}"
RECEIPT_LIMIT="${RECEIPT_LIMIT:-20}"
QUEUE_CAPACITY="${QUEUE_CAPACITY:-128}"
MAINT_START="${MAINT_START:-359070000}"
MAINT_END="${MAINT_END:-359070200}"
ALERTS_OUT="${ALERTS_OUT:-alerts.ndjson}"
ALERT_ACK_TX_IN="${ALERT_ACK_TX_IN:-}"

args=(
  --rpc-url "$HAMN_RPC_URL"
  --start-block "$START_BLOCK"
  --follow
  --fetch-logs
  --receipt-limit "$RECEIPT_LIMIT"
  --extract-features
  --enable-memory
  --enable-sequences
  --enable-alerts
  --alert-dedupe-by-tx true
  --alerts-out "$ALERTS_OUT"
  --alert-maintenance-start-block "$MAINT_START"
  --alert-maintenance-end-block "$MAINT_END"
  --alert-report-interval-blocks 100
  --pipeline-queue-capacity "$QUEUE_CAPACITY"
  --poll-interval-ms "$POLL_INTERVAL_MS"
  --heartbeat-interval-blocks 100
  --snapshot-interval-blocks 500
  --error-mode fail-soft
  --log-format json
  --emit-metrics-json
  --memory-snapshot-out memory_snapshot.json
)

if [ -n "$ALERT_ACK_TX_IN" ]; then
  args+=(--alert-ack-tx-in "$ALERT_ACK_TX_IN")
fi

exec cargo run --release -- "${args[@]}"
