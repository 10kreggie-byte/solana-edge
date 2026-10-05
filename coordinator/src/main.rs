use anyhow::{bail, Context, Result};
use solana_edge_coordinator::create_job;
use solana_edge_rpc::{SolanaRpcClient, DEFAULT_DEVNET_RPC};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let cluster = env::var("SOLANA_EDGE_CLUSTER").unwrap_or_else(|_| "devnet".to_string());
    if cluster != "devnet" {
        bail!("MVP safety guard: SOLANA_EDGE_CLUSTER must be devnet");
    }

    let rpc_url = env::var("SOLANA_RPC_URL").unwrap_or_else(|_| DEFAULT_DEVNET_RPC.to_string());
    let rpc = SolanaRpcClient::new(rpc_url);

    let requested_slot = env::args()
        .nth(1)
        .map(|value| {
            value
                .parse::<u64>()
                .with_context(|| format!("invalid slot: {value}"))
        })
        .transpose()?;

    let job = create_job(&rpc, &cluster, requested_slot).await?;

    eprintln!(
        "created {} from finalized slot {} ({} signatures)",
        job.job_id,
        job.source_slot,
        job.signatures.len()
    );
    println!("{}", serde_json::to_string_pretty(&job)?);

    Ok(())
}
