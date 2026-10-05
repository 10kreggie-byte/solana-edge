# Solana Edge

Solana Edge is an experimental proof-of-useful-edge network for Solana.

The goal is to let low-end computers and web browsers contribute small, verifiable pieces of useful work—starting with deterministic Solana data processing—and earn rewards only when that work is independently verified.

## MVP goal

Prove this loop on Solana Devnet:

```
Browser wallet
  -> signs one worker-authentication message
  -> receives short-lived worker session
  -> browser receives finalized Solana data job
  -> Rust/WASM computes deterministic result
  -> verifier refetches canonical finalized data
  -> coordinator signs wallet-bound WorkReceipt
  -> reward program settles test reward on Devnet (next milestone)
```

## Design principles

- Useful work before token speculation
- Low-end hardware first
- Deterministic, auditable jobs
- No hidden background mining
- Explicit start/stop controls
- Wallet authentication without requesting a transaction
- Off-chain microjob accounting, on-chain settlement
- Devnet before Mainnet
- No real-money token until the protocol works

## Repository layout

```
solana-edge/
├── docs/                 Protocol and architecture specifications
├── shared/
│   ├── rpc/              Minimal finalized Solana JSON-RPC client
│   └── protocol/         Wallet auth + signed work receipts
├── miner/
│   ├── core/             Deterministic Rust job engine
│   ├── cli/              Native miner smoke-test client
│   └── wasm/             Browser/WebAssembly interface
├── coordinator/          Job creation + browser-facing API
├── verifier/             Independent source + result verification
├── programs/
│   └── rewards/          Solana/Anchor reward-settlement program
├── scripts/              Devnet helpers and smoke tests
└── web/                  Browser miner dashboard
```

## Run the native smoke test

```bash
bash scripts/devnet-smoke.sh
```

## Run the browser miner locally

Generate a local receipt-signing key and start the coordinator API:

```bash
eval "$(bash scripts/generate-dev-receipt-key.sh)"
cargo run -p solana-edge-coordinator --bin server
```

In a second terminal:

```bash
cd web
npm install
npm run dev
```

Open `http://127.0.0.1:5173`, connect a Devnet-capable Solana wallet, and press **Start contributing**. The wallet signs an authentication message only. The prototype does not request an on-chain transaction.

The coordinator API binds to `127.0.0.1:8787` by default and only allows `http://localhost:5173` as its browser origin unless overridden with `SOLANA_EDGE_WEB_ORIGIN`.

## Receipt security model

A WorkReceipt is not created from miner self-reporting alone. The coordinator:

1. proves the browser controls the submitted Solana wallet through message signing;
2. independently refetches the finalized source block;
3. recomputes the deterministic miner result;
4. binds the receipt to wallet, job, slot, commitment, score, and reward epoch;
5. signs the receipt with a separate Ed25519 coordinator key.

The coordinator signing seed must be provided through `SOLANA_EDGE_RECEIPT_SIGNING_KEY_HEX` and is never committed to this repository.

## Phase 0 checklist

- [x] Canonical deterministic job format
- [x] Rust job engine
- [x] WebAssembly boundary
- [x] Real finalized Devnet ingestion
- [x] Independent canonical-source verification
- [x] Browser miner UI
- [x] Wallet proof-of-control session
- [x] Signed wallet-bound WorkReceipt
- [ ] Devnet reward settlement program
- [ ] Persistent receipt/replay database
- [ ] Multi-provider verification

## Non-goals for MVP

- Mainnet deployment
- Public token sale
- Liquidity pools
- Governance
- Staking
- Transfer taxes
- GPU/ASIC proof-of-work
- Hidden/background browser mining

## Status

Early engineering prototype. Work receipts currently have test/accounting meaning only and no financial value.
