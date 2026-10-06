# Devnet Deployment and Claim Runbook

This runbook is intentionally Devnet-only.

## Security boundary

Use a dedicated deployer keypair that has no Mainnet funds and is never reused for a production treasury. The reward token created by this flow is disposable and has zero decimals so one raw token unit equals one MVP reward score unit. After the bootstrap supply is deposited into the vault, mint authority is permanently revoked so the Devnet supply is fixed.

Never commit either the deployer keypair or the program keypair. The repository ignores local deployment outputs and keypair JSON files.

## Required tools

- Solana / Agave CLI 4.1.2
- Anchor CLI 1.2.0
- Rust toolchain
- Node.js 24

The workspace pins the Anchor/Solana toolchain in `Anchor.toml`. Current Anchor documentation supports `anchor keys sync` for synchronizing the program key and `anchor deploy` for cluster deployment.

## Local deployment

Fund the dedicated Devnet wallet with enough Devnet SOL for program deployment, then run:

```bash
export SOLANA_WALLET=~/.config/solana/id.json
export SOLANA_RPC_URL=https://api.devnet.solana.com
bash scripts/devnet/deploy-rewards.sh
```

The script:

1. loads or creates `target/deploy/solana_edge_rewards-keypair.json`;
2. runs `anchor keys sync`;
3. builds and deploys the program;
4. creates a disposable standard SPL mint with 0 decimals;
5. derives the config PDA;
6. creates the PDA-owned associated reward vault;
7. initializes the reward program;
8. mints the fixed test supply into the vault;
9. permanently revokes the test mint authority;
10. writes `devnet-deployment.json`.

After the first successful deployment, securely back up the program keypair. Recreating it changes the program ID.

## GitHub Actions deployment

The repository includes the manual `Deploy Devnet Rewards` workflow. Before running it, add these repository/environment secrets:

- `SOLANA_DEVNET_DEPLOYER_KEYPAIR_JSON`: JSON byte array for a dedicated funded Devnet wallet.
- `SOLANA_EDGE_PROGRAM_KEYPAIR_JSON`: JSON byte array for the persistent reward-program keypair.

The workflow never generates Mainnet assets. It uploads `devnet-deployment.json` as an artifact containing public addresses only.

## Close and publish an epoch

The coordinator stores verified work receipts in SQLite. Build a deterministic manifest:

```bash
bash scripts/rewards/build-epoch.sh <reward_epoch>
```

This writes `reward-manifests/<reward_epoch>.json`.

Publish its root:

```bash
export SOLANA_EDGE_REWARDS_PROGRAM_ID=<program id>
export SOLANA_WALLET=~/.config/solana/id.json
node ops/publish-epoch.mjs reward-manifests/<reward_epoch>.json
```

Epoch publication is one-way in V1: the PDA for that reward epoch can be initialized only once.

## Serve proofs to miners

Start the coordinator with:

```bash
export SOLANA_EDGE_RECEIPT_DB=solana-edge-receipts.sqlite3
export SOLANA_EDGE_REWARD_MANIFEST_DIR=reward-manifests
export SOLANA_EDGE_REWARDS_PROGRAM_ID=<program id>
export SOLANA_EDGE_REWARD_MINT=<mint>
cargo run -p solana-edge-coordinator --bin server
```

The browser retrieves only the connected wallet's claim proof from:

```
GET /v1/rewards/{reward_epoch}/{worker_pubkey}
```

## Browser claim

The browser independently derives:

- config PDA: `["config"]`
- epoch PDA: `["epoch", reward_epoch_le_u64]`
- claim PDA: `["claim", reward_epoch_le_u64, worker_pubkey]`
- PDA-owned reward vault ATA
- worker reward-token ATA

It then asks the wallet to sign a transaction containing the Anchor `claim` instruction. If the worker token account does not exist, an idempotent ATA-creation instruction is prepended.

A second claim for the same wallet and epoch fails because the claim PDA already exists.

## Before any Mainnet discussion

Do not move this program or token economics to Mainnet until all of the following exist:

- third-party smart-contract audit;
- resolved frontend dependency audit findings;
- multi-provider canonical data verification;
- authority/multisig design;
- epoch dispute/challenge process;
- deterministic cross-language Merkle test vectors;
- load and abuse testing;
- explicit emission economics and treasury controls.
