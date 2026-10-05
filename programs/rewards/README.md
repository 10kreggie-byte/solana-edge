# Rewards Program

Solana Edge Reward Settlement V1 uses an Anchor program and a standard SPL test token on Devnet.

## State

- `RewardConfig` PDA: authority, reward mint, vault, pause flag.
- `RewardEpoch` PDA: epoch number, Merkle root, committed amount, claimed amount.
- `ClaimRecord` PDA: one immutable claim record per wallet per epoch.

## Settlement flow

1. The coordinator persists independently verified signed receipts to SQLite.
2. `solana-edge-settlement` aggregates receipt score by wallet for one reward epoch.
3. The settlement CLI emits a deterministic Merkle root and per-wallet proofs.
4. Protocol authority publishes the epoch root on Solana.
5. The worker submits `amount + proof`.
6. The program recomputes the leaf, verifies the directional Merkle proof, creates the claim PDA, and transfers SPL tokens from the program-controlled vault.
7. Reusing the same worker/epoch seeds fails because the claim PDA already exists.

## Merkle leaf

```
SHA256(
  "solana-edge/reward-leaf/v0"
  || reward_epoch_le_u64
  || worker_pubkey_32_bytes
  || amount_le_u64
)
```

Parent nodes are:

```
SHA256(
  "solana-edge/reward-node/v0"
  || left_32_bytes
  || right_32_bytes
)
```

Direction is explicit in each proof node.

## Safety controls

- authority-only epoch publication;
- pause switch;
- reward vault must be owned by the config PDA;
- reward mint and destination mint are constrained;
- epoch claimed amount cannot exceed committed total;
- zero-value claims are rejected;
- one claim PDA per wallet per epoch prevents double claims permanently.

## Devnet only

The program ID currently committed in `declare_id!` is a development placeholder. Before the first Devnet deployment, create the program keypair and run `anchor keys sync` so the source ID matches the deploy keypair.

No Mainnet deployment or financially valuable token is authorized by this codebase.
