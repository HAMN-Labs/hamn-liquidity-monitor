#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "Usage: $0 <features.ndjson> [more_files...]" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for this script" >&2
  exit 1
fi

TOTAL_LINES=$(cat "$@" | wc -l | tr -d ' ')
VALID_JSON=$(cat "$@" | jq -c . >/dev/null 2>&1 && cat "$@" | wc -l | tr -d ' ' || true)

echo "total_lines=$TOTAL_LINES"
if [ -n "${VALID_JSON:-}" ]; then
  echo "valid_json_lines=$VALID_JSON"
fi

cat "$@" | jq -r '.event // "unknown"' | sort | uniq -c | awk '{print "event_"$2"="$1}'

cat "$@" | jq -r '.pool // "unknown"' | sort | uniq -c | sort -nr | head -n 10 | awk '{print "top_pool count="$1" pool="$2}'
