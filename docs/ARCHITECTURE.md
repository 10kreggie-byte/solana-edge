# Solana Edge Architecture

## MVP invariant

A reward is earned only when a miner returns the canonical result for a deterministic job and that result is independently verified.

## Data flow

1. Coordinator selects finalized Solana input.
2. Coordinator emits a canonical `JobSpec`.
3. Miner executes the job locally.
4. Miner returns `JobResult` and a SHA-256 commitment.
5. Verifier independently recomputes the job.
6. Matching commitments produce a signed `WorkReceipt`.
7. Receipts are accumulated into an epoch.
8. The reward program settles authorized Devnet claims.

## Trust model

For MVP, the coordinator is trusted for job assignment but not trusted for miner correctness. Miner outputs are deterministic and reproducible. Later phases replace single-coordinator trust with multiple schedulers, randomized audits, and threshold-signed reward roots.

## Canonical job v0

The first job type is deliberately small:

- Input: an ordered list of finalized Solana transaction signatures.
- Operation: normalize, sort, count, and hash the canonical byte representation.
- Output: count plus SHA-256 digest.

The first implementation uses fixture data. Live Solana RPC ingestion is added after deterministic equivalence is proven across native Rust and WebAssembly.

## Security boundaries

- Browser code never receives private wallet keys.
- Wallet signing is delegated to the wallet provider.
- CPU usage must be opt-in and user-configurable.
- Work receipts include job id, worker pubkey, result commitment, score, epoch, and coordinator signature.
- On-chain reward settlement must prevent replay/double-claim.
