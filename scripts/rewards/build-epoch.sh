#!/usr/bin/env bash
set -euo pipefail

EPOCH="${1:-}"
DB="${SOLANA_EDGE_RECEIPT_DB:-solana-edge-receipts.sqlite3}"
OUT_DIR="${SOLANA_EDGE_REWARD_MANIFEST_DIR:-reward-manifests}"

if [[ -z "$EPOCH" ]]; then
  echo "usage: bash scripts/rewards/build-epoch.sh <reward_epoch>" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
cargo run -q -p solana-edge-settlement -- "$DB" "$EPOCH" > "$OUT_DIR/$EPOCH.json"

echo "$OUT_DIR/$EPOCH.json"
