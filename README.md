# Solana Edge

Solana Edge is an experimental proof-of-useful-edge network for Solana.

The goal is to let low-end computers and web browsers contribute small, verifiable pieces of useful work—starting with deterministic Solana data processing—and earn rewards only when that work is independently verified.

## MVP goal

Prove this loop on Solana Devnet:

```
Browser/native miner
  -> receives deterministic finalized Solana data job
  -> processes job in shared Rust/WASM engine
  -> returns result + commitment
  -> independent verifier refetches canonical Solana source data
  -> verifier recomputes result and requires an exact match
  -> coordinator issues signed work receipt (next milestone)
  -> reward program settles test reward on Devnet
```

## Design principles

- Useful work before token speculation
- Low-end hardware first
- Deterministic, auditable jobs
- No hidden background mining
- Explicit CPU/storage/bandwidth controls
- Off-chain microjob accounting, on-chain settlement
- Devnet before Mainnet
- Fixed security boundaries and minimal smart-contract surface
- No real-money token until the protocol works

## Repository layout

```
solana-edge/
├── docs/                 Protocol and architecture specifications
├── shared/
│   └── rpc/              Minimal finalized Solana JSON-RPC client
├── miner/
│   ├── core/             Deterministic Rust job engine
│   ├── cli/              Native miner smoke-test client
│   └── wasm/             Browser/WebAssembly interface
├── coordinator/          Finalized Devnet job creation
├── verifier/             Independent source + result verification
├── programs/
│   └── rewards/          Solana/Anchor reward-settlement program
├── scripts/              End-to-end Devnet smoke tests
└── web/                  Browser miner dashboard
```

## Current Devnet pipeline

The coordinator currently uses Solana's JSON-RPC interface with `finalized` commitment. It chooses a recently produced finalized block and requests signature-only transaction data. The generated `JobSpec` is cryptographically bound to the cluster, slot, blockhash, job id, and transaction signatures.

The verifier independently refetches the same finalized slot and rejects the job if its blockhash or signature set does not match the canonical RPC response.

Public Devnet RPC is suitable only for this prototype and can rate-limit callers. Production architecture will use multiple independent RPC sources.

## Run the native end-to-end smoke test

Requirements: stable Rust/Cargo and internet access.

```bash
bash scripts/devnet-smoke.sh
```

Equivalent manual flow:

```bash
cargo run -p solana-edge-coordinator > /tmp/job.json
cargo run -p solana-edge-miner-cli -- /tmp/job.json > /tmp/result.json
cargo run -p solana-edge-verifier -- /tmp/job.json /tmp/result.json
```

To reproduce a specific finalized slot:

```bash
cargo run -p solana-edge-coordinator -- 123456789 > /tmp/job.json
```

Override the RPC endpoint with:

```bash
SOLANA_RPC_URL=https://your-devnet-rpc.example bash scripts/devnet-smoke.sh
```

## Phase 0 checklist

- [x] Define a canonical deterministic job format.
- [x] Build the Rust job engine.
- [x] Compile boundary for WebAssembly.
- [x] Pull real finalized Devnet data.
- [x] Add independent canonical-source verification.
- [ ] Run the browser WASM miner end-to-end.
- [ ] Sign a work receipt bound to miner wallet + verified result.
- [ ] Settle a test reward on Devnet.

## Non-goals for MVP

- Mainnet deployment
- Public token sale
- Liquidity pools
- Governance
- Staking
- Transfer taxes
- GPU/ASIC proof-of-work
- Unbounded browser resource usage

## Status

Early engineering prototype. Nothing in this repository should be treated as production-ready or financially valuable.
