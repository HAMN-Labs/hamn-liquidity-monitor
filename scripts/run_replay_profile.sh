#!/usr/bin/env bash
set -euo pipefail

: "${HAMN_RPC_URL:?HAMN_RPC_URL is required}"

START_BLOCK="${START_BLOCK:-359066951}"
END_BLOCK="${END_BLOCK:-359067051}"
RECEIPT_LIMIT="${RECEIPT_LIMIT:-20}"
QUEUE_CAPACITY="${QUEUE_CAPACITY:-128}"

exec cargo run --release -- \
  --rpc-url "$HAMN_RPC_URL" \
  --start-block "$START_BLOCK" \
  --end-block "$END_BLOCK" \
  --fetch-logs \
  --receipt-limit "$RECEIPT_LIMIT" \
  --extract-features \
  --enable-memory \
  --enable-sequences \
  --pipeline-queue-capacity "$QUEUE_CAPACITY" \
  --log-format json \
  --emit-metrics-json \
  --memory-snapshot-out memory_snapshot.json
