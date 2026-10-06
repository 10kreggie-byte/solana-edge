#!/usr/bin/env bash
set -euo pipefail

export SOLANA_RPC_URL="${SOLANA_RPC_URL:-https://api.devnet.solana.com}"
export SOLANA_WALLET="${SOLANA_WALLET:-$HOME/.config/solana/id.json}"
export ANCHOR_PROVIDER_URL="$SOLANA_RPC_URL"
export ANCHOR_WALLET="$SOLANA_WALLET"

if [[ ! -f "$SOLANA_WALLET" ]]; then
  echo "Missing Devnet deployer keypair: $SOLANA_WALLET" >&2
  exit 1
fi

bash scripts/devnet/prepare-program-keypair.sh

DEVNET_GENESIS_HASH="EtWTRABZaYq6iMfeYKouRu166VU2xqa1"
CONNECTED_GENESIS_HASH="$(solana genesis-hash --url "$SOLANA_RPC_URL")"
if [[ "$CONNECTED_GENESIS_HASH" != "$DEVNET_GENESIS_HASH" ]]; then
  echo "Refusing deployment: RPC is not Solana Devnet." >&2
  echo "Expected genesis: $DEVNET_GENESIS_HASH" >&2
  echo "Connected genesis: $CONNECTED_GENESIS_HASH" >&2
  exit 1
fi

echo "Using RPC: $SOLANA_RPC_URL" >&2
echo "Verified Devnet genesis: $CONNECTED_GENESIS_HASH" >&2
echo "Deployer: $(solana address -k "$SOLANA_WALLET")" >&2
echo "Balance: $(solana balance --url "$SOLANA_RPC_URL" -k "$SOLANA_WALLET")" >&2

anchor keys sync
anchor build
anchor deploy

pushd ops >/dev/null
npm install
popd >/dev/null

node ops/bootstrap-devnet.mjs

echo "" >&2
echo "Devnet deployment written to devnet-deployment.json." >&2
echo "Do not reuse the deployer keypair for Mainnet funds." >&2
