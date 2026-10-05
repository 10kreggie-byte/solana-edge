#!/usr/bin/env bash
set -euo pipefail

if ! command -v openssl >/dev/null 2>&1; then
  echo "openssl is required" >&2
  exit 1
fi

echo "export SOLANA_EDGE_RECEIPT_SIGNING_KEY_HEX=$(openssl rand -hex 32)"
