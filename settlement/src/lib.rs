use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const LEAF_DOMAIN: &[u8] = b"solana-edge/reward-leaf/v0";
const NODE_DOMAIN: &[u8] = b"solana-edge/reward-node/v0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardProofNode {
    pub hash_hex: String,
    pub sibling_is_left: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardClaim {
    pub worker_pubkey: String,
    pub amount: u64,
    pub proof: Vec<RewardProofNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardManifest {
    pub version: u8,
    pub reward_epoch: u64,
    pub merkle_root_hex: String,
    pub total_amount: u64,
    pub claims: Vec<RewardClaim>,
}

pub fn build_manifest_from_sqlite(path: &str, reward_epoch: u64) -> Result<RewardManifest> {
    let connection =
        Connection::open(path).with_context(|| format!("failed to open receipt DB: {path}"))?;

    let mut statement = connection.prepare(
        "SELECT worker_pubkey, SUM(score)
         FROM receipts
         WHERE reward_epoch = ?1
         GROUP BY worker_pubkey
         ORDER BY worker_pubkey ASC",
    )?;

    let rows = statement.query_map(params![reward_epoch], |row| {
        let worker_pubkey: String = row.get(0)?;
        let amount: i64 = row.get(1)?;
        Ok((worker_pubkey, amount))
    })?;

    let mut rewards = BTreeMap::new();

    for row in rows {
        let (worker, amount) = row?;
        if amount <= 0 {
            continue;
        }
        let amount = u64::try_from(amount).context("aggregated reward amount overflow")?;
        rewards.insert(worker, amount);
    }

    build_manifest(reward_epoch, rewards)
}

pub fn build_manifest(
    reward_epoch: u64,
    rewards: BTreeMap<String, u64>,
) -> Result<RewardManifest> {
    if rewards.is_empty() {
        bail!("cannot build an empty reward epoch");
    }

    let mut workers = Vec::with_capacity(rewards.len());
    let mut leaves = Vec::with_capacity(rewards.len());
    let mut total_amount = 0u64;

    for (worker, amount) in rewards {
        if amount == 0 {
            bail!("zero-value reward for worker {worker}");
        }

        let pubkey = decode_pubkey(&worker)?;
        total_amount = total_amount
            .checked_add(amount)
            .context("total reward amount overflow")?;

        workers.push((worker, amount));
        leaves.push(reward_leaf_hash(reward_epoch, &pubkey, amount));
    }

    let (root, proofs) = build_tree_and_proofs(&leaves);

    let claims = workers
        .into_iter()
        .zip(proofs)
        .map(|((worker_pubkey, amount), proof)| RewardClaim {
            worker_pubkey,
            amount,
            proof: proof
                .into_iter()
                .map(|node| RewardProofNode {
                    hash_hex: hex::encode(node.hash),
                    sibling_is_left: node.sibling_is_left,
                })
                .collect(),
        })
        .collect();

    Ok(RewardManifest {
        version: 0,
        reward_epoch,
        merkle_root_hex: hex::encode(root),
        total_amount,
        claims,
    })
}

#[derive(Debug, Clone, Copy)]
struct ProofNode {
    hash: [u8; 32],
    sibling_is_left: bool,
}

fn build_tree_and_proofs(leaves: &[[u8; 32]]) -> ([u8; 32], Vec<Vec<ProofNode>>) {
    if leaves.len() == 1 {
        return (leaves[0], vec![Vec::new()]);
    }

    let mut proofs = vec![Vec::new(); leaves.len()];
    let mut level: Vec<([u8; 32], Vec<usize>)> = leaves
        .iter()
        .copied()
        .enumerate()
        .map(|(index, hash)| (hash, vec![index]))
        .collect();

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));

        for chunk in level.chunks(2) {
            let (left_hash, left_indices) = &chunk[0];
            let (right_hash, right_indices) = if chunk.len() == 2 {
                (&chunk[1].0, chunk[1].1.as_slice())
            } else {
                (left_hash, left_indices.as_slice())
            };

            for index in left_indices {
                proofs[*index].push(ProofNode {
                    hash: *right_hash,
                    sibling_is_left: false,
                });
            }

            if chunk.len() == 2 {
                for index in right_indices {
                    proofs[*index].push(ProofNode {
                        hash: *left_hash,
                        sibling_is_left: true,
                    });
                }
            } else {
                for index in left_indices {
                    if let Some(last) = proofs[*index].last_mut() {
                        last.sibling_is_left = false;
                    }
                }
            }

            let mut parent_indices = left_indices.clone();
            if chunk.len() == 2 {
                parent_indices.extend_from_slice(right_indices);
            }

            next.push((reward_node_hash(left_hash, right_hash), parent_indices));
        }

        level = next;
    }

    (level[0].0, proofs)
}

pub fn reward_leaf_hash(reward_epoch: u64, worker: &[u8; 32], amount: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(LEAF_DOMAIN);
    hasher.update(reward_epoch.to_le_bytes());
    hasher.update(worker);
    hasher.update(amount.to_le_bytes());
    hasher.finalize().into()
}

pub fn reward_node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(NODE_DOMAIN);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn decode_pubkey(value: &str) -> Result<[u8; 32]> {
    let bytes = bs58::decode(value)
        .into_vec()
        .with_context(|| format!("invalid base58 worker pubkey: {value}"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("worker pubkey is not 32 bytes: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_leaf_uses_empty_proof() {
        let mut rewards = BTreeMap::new();
        rewards.insert(
            bs58::encode([1u8; 32]).into_string(),
            100,
        );

        let manifest = build_manifest(9, rewards).unwrap();
        assert_eq!(manifest.claims.len(), 1);
        assert!(manifest.claims[0].proof.is_empty());
    }

    #[test]
    fn manifests_are_order_deterministic() {
        let first = bs58::encode([1u8; 32]).into_string();
        let second = bs58::encode([2u8; 32]).into_string();

        let mut a = BTreeMap::new();
        a.insert(second.clone(), 20);
        a.insert(first.clone(), 10);

        let mut b = BTreeMap::new();
        b.insert(first, 10);
        b.insert(second, 20);

        assert_eq!(
            build_manifest(42, a).unwrap(),
            build_manifest(42, b).unwrap()
        );
    }
}
