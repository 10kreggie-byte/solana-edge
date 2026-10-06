#!/usr/bin/env bash
set -euo pipefail

KEYPAIR="target/deploy/solana_edge_rewards-keypair.json"
mkdir -p target/deploy

if [[ -n "${SOLANA_EDGE_PROGRAM_KEYPAIR_JSON:-}" ]]; then
  printf '%s\n' "$SOLANA_EDGE_PROGRAM_KEYPAIR_JSON" > "$KEYPAIR"
  chmod 600 "$KEYPAIR"
  echo "Loaded program keypair from SOLANA_EDGE_PROGRAM_KEYPAIR_JSON." >&2
elif [[ ! -f "$KEYPAIR" ]]; then
  solana-keygen new     --no-bip39-passphrase     --silent     --outfile "$KEYPAIR"
  chmod 600 "$KEYPAIR"
  echo "Generated NEW Devnet program keypair at $KEYPAIR." >&2
  echo "Back this file up securely before deployment." >&2
else
  echo "Using existing program keypair at $KEYPAIR." >&2
fi

echo "Program ID: $(solana address -k "$KEYPAIR")"
