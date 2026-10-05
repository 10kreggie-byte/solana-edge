# Solana Edge Architecture

## MVP invariant

A reward is earned only when a miner returns the canonical result for a deterministic job and that result is independently verified against finalized Solana data.

## Data flow

1. Coordinator asks an RPC source for a finalized Solana block.
2. Coordinator emits a canonical `JobSpec` bound to cluster, slot, blockhash, and transaction signatures.
3. Miner executes the job locally using the shared Rust engine.
4. Browser miners call the same engine through WebAssembly.
5. Miner returns a `JobResult` containing a SHA-256 commitment.
6. Verifier independently fetches the source slot from RPC.
7. Verifier checks blockhash and transaction signatures, then recomputes the result.
8. Only an exact match is eligible for a signed `WorkReceipt`.
9. Receipts will later be accumulated into reward epochs for Devnet settlement.

## Canonical job v0

The first useful-work primitive is intentionally small and deterministic:

- source: one finalized Solana block;
- input: cluster id, slot, blockhash, and transaction signatures;
- operation: sort signatures, encode fields with explicit length prefixes, and hash the canonical byte representation;
- output: item count plus SHA-256 commitment.

The commitment is domain-separated and bound to the job id and source block, reducing replay ambiguity between jobs.

## Current trust model

The coordinator is trusted to assign work, but it is not trusted to decide whether a miner is correct. The verifier checks the coordinator's claimed source data against finalized Solana RPC data before accepting a result.

For the MVP, public Devnet RPC is the default upstream. Production must use multiple independent providers and must tolerate rate limits, outages, and inconsistent providers.

## Security boundaries

- Browser code never receives wallet private keys.
- Wallet signing is delegated to a wallet provider.
- CPU, storage, and bandwidth contribution must be opt-in and user-configurable.
- No reward is authorized from a miner's self-reported work alone.
- Work receipts will bind job id, worker pubkey, result commitment, score, epoch, and coordinator authorization.
- On-chain reward settlement must prevent replay and double-claim.
- Microjob traffic stays off-chain; Solana is used for settlement.

## Local Devnet smoke test

```bash
cargo run -p solana-edge-coordinator > /tmp/job.json
cargo run -p solana-edge-miner-cli -- /tmp/job.json > /tmp/result.json
cargo run -p solana-edge-verifier -- /tmp/job.json /tmp/result.json
```

Set `SOLANA_RPC_URL` to override the default `https://api.devnet.solana.com`.
