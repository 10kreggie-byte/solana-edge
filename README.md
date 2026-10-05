# Solana Edge

Solana Edge is an experimental proof-of-useful-edge network for Solana.

The goal is to let low-end computers and web browsers contribute small, verifiable pieces of useful work—starting with deterministic Solana data processing—and earn rewards only when that work is independently verified.

## MVP goal

Prove this loop on Solana Devnet:

```
Browser miner
  -> receives deterministic Solana data job
  -> processes job in Rust/WASM
  -> returns result + commitment
  -> independent verifier checks result
  -> coordinator issues signed work receipt
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

## Initial repository layout

```
solana-edge/
├── docs/                 Protocol and architecture specifications
├── miner/
│   ├── core/             Deterministic Rust job engine
│   └── wasm/             Browser/WebAssembly interface
├── coordinator/          Job assignment and work receipts
├── verifier/             Independent deterministic verification
├── programs/
│   └── rewards/          Solana/Anchor reward-settlement program
└── web/                  Browser miner dashboard
```

## Phase 0

1. Define a canonical deterministic job format.
2. Build the Rust job engine.
3. Compile it to WebAssembly.
4. Run the same job in miner and verifier and require identical output.
5. Sign a work receipt.
6. Settle a test reward on Devnet.

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

Early engineering scaffold. Nothing in this repository should be treated as production-ready or financially valuable.
