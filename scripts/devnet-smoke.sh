#!/usr/bin/env bash
set -euo pipefail

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "1/3 creating finalized Devnet job" >&2
cargo run -q -p solana-edge-coordinator > "$tmp_dir/job.json"

echo "2/3 executing deterministic miner" >&2
cargo run -q -p solana-edge-miner-cli -- "$tmp_dir/job.json" > "$tmp_dir/result.json"

echo "3/3 independently verifying source + result" >&2
cargo run -q -p solana-edge-verifier -- "$tmp_dir/job.json" "$tmp_dir/result.json"
