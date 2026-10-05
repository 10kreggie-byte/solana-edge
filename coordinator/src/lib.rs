pub mod receipt_store;

use anyhow::Result;
use solana_edge_miner_core::JobSpec;
use solana_edge_rpc::SolanaRpcClient;

pub async fn create_job(
    rpc: &SolanaRpcClient,
    cluster: &str,
    requested_slot: Option<u64>,
) -> Result<JobSpec> {
    let block = match requested_slot {
        Some(slot) => rpc.finalized_block(slot).await?,
        None => rpc.latest_finalized_block().await?,
    };

    let prefix: String = block.blockhash.chars().take(12).collect();

    Ok(JobSpec {
        version: 0,
        job_id: format!("{cluster}-slot-{}-{prefix}", block.slot),
        source_cluster: cluster.to_string(),
        source_slot: block.slot,
        source_blockhash: block.blockhash,
        signatures: block.signatures,
    })
}
