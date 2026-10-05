use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

declare_id!("1thX6LZfHDZZKUs92febYZhYRcXddmzfzF2NvTkPNE");

const CONFIG_SEED: &[u8] = b"config";
const EPOCH_SEED: &[u8] = b"epoch";
const CLAIM_SEED: &[u8] = b"claim";
const LEAF_DOMAIN: &[u8] = b"solana-edge/reward-leaf/v0";
const NODE_DOMAIN: &[u8] = b"solana-edge/reward-node/v0";

#[program]
pub mod solana_edge_rewards {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.reward_mint = ctx.accounts.reward_mint.key();
        config.vault = ctx.accounts.vault.key();
        config.bump = ctx.bumps.config;
        config.paused = false;
        Ok(())
    }

    pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
        ctx.accounts.config.paused = paused;
        Ok(())
    }

    pub fn publish_epoch(
        ctx: Context<PublishEpoch>,
        reward_epoch: u64,
        merkle_root: [u8; 32],
        total_amount: u64,
    ) -> Result<()> {
        require!(total_amount > 0, RewardError::ZeroAmount);

        let epoch = &mut ctx.accounts.epoch;
        epoch.reward_epoch = reward_epoch;
        epoch.merkle_root = merkle_root;
        epoch.total_amount = total_amount;
        epoch.claimed_amount = 0;
        epoch.published_at = Clock::get()?.unix_timestamp;
        epoch.bump = ctx.bumps.epoch;
        Ok(())
    }

    pub fn claim(
        ctx: Context<Claim>,
        reward_epoch: u64,
        amount: u64,
        proof: Vec<MerkleProofNode>,
    ) -> Result<()> {
        require!(!ctx.accounts.config.paused, RewardError::Paused);
        require!(amount > 0, RewardError::ZeroAmount);
        require!(
            ctx.accounts.epoch.reward_epoch == reward_epoch,
            RewardError::EpochMismatch
        );

        let worker = ctx.accounts.worker.key();
        let leaf = reward_leaf_hash(reward_epoch, &worker, amount);
        require!(
            verify_merkle_proof(leaf, &proof, ctx.accounts.epoch.merkle_root),
            RewardError::InvalidProof
        );

        let new_claimed = ctx
            .accounts
            .epoch
            .claimed_amount
            .checked_add(amount)
            .ok_or(RewardError::MathOverflow)?;
        require!(
            new_claimed <= ctx.accounts.epoch.total_amount,
            RewardError::EpochOverdrawn
        );

        let signer_seeds: &[&[u8]] = &[CONFIG_SEED, &[ctx.accounts.config.bump]];
        let signer = &[signer_seeds];

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.vault.to_account_info(),
            mint: ctx.accounts.reward_mint.to_account_info(),
            to: ctx.accounts.worker_token_account.to_account_info(),
            authority: ctx.accounts.config.to_account_info(),
        };

        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                cpi_accounts,
                signer,
            ),
            amount,
            ctx.accounts.reward_mint.decimals,
        )?;

        ctx.accounts.epoch.claimed_amount = new_claimed;

        let claim_record = &mut ctx.accounts.claim_record;
        claim_record.reward_epoch = reward_epoch;
        claim_record.worker = worker;
        claim_record.amount = amount;
        claim_record.claimed_at = Clock::get()?.unix_timestamp;
        claim_record.bump = ctx.bumps.claim_record;

        emit!(RewardClaimed {
            reward_epoch,
            worker,
            amount,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + RewardConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, RewardConfig>,

    pub reward_mint: Account<'info, Mint>,

    #[account(
        constraint = vault.mint == reward_mint.key() @ RewardError::VaultMintMismatch,
        constraint = vault.owner == config.key() @ RewardError::VaultAuthorityMismatch
    )]
    pub vault: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority
    )]
    pub config: Account<'info, RewardConfig>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(reward_epoch: u64)]
pub struct PublishEpoch<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority
    )]
    pub config: Account<'info, RewardConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + RewardEpoch::INIT_SPACE,
        seeds = [EPOCH_SEED, &reward_epoch.to_le_bytes()],
        bump
    )]
    pub epoch: Account<'info, RewardEpoch>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(reward_epoch: u64)]
pub struct Claim<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = reward_mint,
        has_one = vault
    )]
    pub config: Account<'info, RewardConfig>,

    #[account(
        mut,
        seeds = [EPOCH_SEED, &reward_epoch.to_le_bytes()],
        bump = epoch.bump
    )]
    pub epoch: Account<'info, RewardEpoch>,

    #[account(
        init,
        payer = worker,
        space = 8 + ClaimRecord::INIT_SPACE,
        seeds = [
            CLAIM_SEED,
            &reward_epoch.to_le_bytes(),
            worker.key().as_ref()
        ],
        bump
    )]
    pub claim_record: Account<'info, ClaimRecord>,

    #[account(mut)]
    pub worker: Signer<'info>,

    pub reward_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault.mint == reward_mint.key() @ RewardError::VaultMintMismatch,
        constraint = vault.owner == config.key() @ RewardError::VaultAuthorityMismatch
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = worker_token_account.mint == reward_mint.key() @ RewardError::DestinationMintMismatch,
        constraint = worker_token_account.owner == worker.key() @ RewardError::DestinationOwnerMismatch
    )]
    pub worker_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct RewardConfig {
    pub authority: Pubkey,
    pub reward_mint: Pubkey,
    pub vault: Pubkey,
    pub bump: u8,
    pub paused: bool,
}

#[account]
#[derive(InitSpace)]
pub struct RewardEpoch {
    pub reward_epoch: u64,
    pub merkle_root: [u8; 32],
    pub total_amount: u64,
    pub claimed_amount: u64,
    pub published_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct ClaimRecord {
    pub reward_epoch: u64,
    pub worker: Pubkey,
    pub amount: u64,
    pub claimed_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct MerkleProofNode {
    pub hash: [u8; 32],
    pub sibling_is_left: bool,
}

#[event]
pub struct RewardClaimed {
    pub reward_epoch: u64,
    pub worker: Pubkey,
    pub amount: u64,
}

#[error_code]
pub enum RewardError {
    #[msg("reward settlement is paused")]
    Paused,
    #[msg("reward amount must be greater than zero")]
    ZeroAmount,
    #[msg("claim reward epoch does not match the epoch account")]
    EpochMismatch,
    #[msg("Merkle proof is invalid")]
    InvalidProof,
    #[msg("reward arithmetic overflow")]
    MathOverflow,
    #[msg("claim would exceed the epoch's committed reward amount")]
    EpochOverdrawn,
    #[msg("reward vault mint does not match configuration")]
    VaultMintMismatch,
    #[msg("reward vault authority must be the config PDA")]
    VaultAuthorityMismatch,
    #[msg("worker token account mint does not match reward mint")]
    DestinationMintMismatch,
    #[msg("worker token account owner does not match worker")]
    DestinationOwnerMismatch,
}

pub fn reward_leaf_hash(reward_epoch: u64, worker: &Pubkey, amount: u64) -> [u8; 32] {
    hashv(&[
        LEAF_DOMAIN,
        &reward_epoch.to_le_bytes(),
        worker.as_ref(),
        &amount.to_le_bytes(),
    ])
    .to_bytes()
}

pub fn reward_node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    hashv(&[NODE_DOMAIN, left, right]).to_bytes()
}

pub fn verify_merkle_proof(
    leaf: [u8; 32],
    proof: &[MerkleProofNode],
    expected_root: [u8; 32],
) -> bool {
    let mut current = leaf;

    for node in proof {
        current = if node.sibling_is_left {
            reward_node_hash(&node.hash, &current)
        } else {
            reward_node_hash(&current, &node.hash)
        };
    }

    current == expected_root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_merkle_proof_verifies() {
        let worker_a = Pubkey::new_from_array([1u8; 32]);
        let worker_b = Pubkey::new_from_array([2u8; 32]);
        let left = reward_leaf_hash(42, &worker_a, 10);
        let right = reward_leaf_hash(42, &worker_b, 20);
        let root = reward_node_hash(&left, &right);

        let proof = vec![MerkleProofNode {
            hash: right,
            sibling_is_left: false,
        }];

        assert!(verify_merkle_proof(left, &proof, root));
    }

    #[test]
    fn wrong_amount_changes_leaf() {
        let worker = Pubkey::new_from_array([7u8; 32]);
        assert_ne!(
            reward_leaf_hash(42, &worker, 10),
            reward_leaf_hash(42, &worker, 11)
        );
    }
}
