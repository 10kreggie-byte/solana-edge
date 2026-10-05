use anyhow::{bail, Context, Result};
use solana_edge_miner_core::{JobResult, JobSpec};
use solana_edge_rpc::{SolanaRpcClient, DEFAULT_DEVNET_RPC};
use solana_edge_verifier::verify_job_result;
use std::{env, fs};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let job_path = args
        .next()
        .context("usage: solana-edge-verifier <job.json> <result.json>")?;
    let result_path = args
        .next()
        .context("usage: solana-edge-verifier <job.json> <result.json>")?;

    if args.next().is_some() {
        bail!("usage: solana-edge-verifier <job.json> <result.json>");
    }

    let job: JobSpec = serde_json::from_str(
        &fs::read_to_string(&job_path)
            .with_context(|| format!("failed to read job file: {job_path}"))?,
    )
    .with_context(|| format!("invalid JobSpec JSON: {job_path}"))?;

    let miner_result: JobResult = serde_json::from_str(
        &fs::read_to_string(&result_path)
            .with_context(|| format!("failed to read result file: {result_path}"))?,
    )
    .with_context(|| format!("invalid JobResult JSON: {result_path}"))?;

    let rpc_url = env::var("SOLANA_RPC_URL").unwrap_or_else(|_| DEFAULT_DEVNET_RPC.to_string());
    let rpc = SolanaRpcClient::new(rpc_url);
    let verified = verify_job_result(&rpc, &job, &miner_result).await?;

    println!(
        "VERIFIED job={} slot={} signatures={} commitment={}",
        verified.job_id,
        verified.source_slot,
        verified.item_count,
        verified.commitment_hex
    );

    Ok(())
}
