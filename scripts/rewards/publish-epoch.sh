#!/usr/bin/env bash
set -euo pipefail

MANIFEST="${1:-}"
if [[ -z "$MANIFEST" ]]; then
  echo "usage: bash scripts/rewards/publish-epoch.sh <reward-manifest.json>" >&2
  exit 1
fi

if [[ -z "${SOLANA_EDGE_REWARDS_PROGRAM_ID:-}" ]]; then
  echo "SOLANA_EDGE_REWARDS_PROGRAM_ID is required" >&2
  exit 1
fi

pushd ops >/dev/null
npm install
popd >/dev/null

node ops/publish-epoch.mjs "$MANIFEST"
