use anyhow::{bail, Context, Result};
use solana_edge_miner_core::{execute_job, JobResult, JobSpec};
use solana_edge_rpc::{SolanaRpcClient, DEFAULT_DEVNET_RPC};
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

    if job.source_cluster != "devnet" {
        bail!(
            "MVP safety guard: verifier only accepts devnet jobs, received {}",
            job.source_cluster
        );
    }

    let rpc_url = env::var("SOLANA_RPC_URL").unwrap_or_else(|_| DEFAULT_DEVNET_RPC.to_string());
    let rpc = SolanaRpcClient::new(rpc_url);
    let canonical = rpc.finalized_block(job.source_slot).await?;

    if canonical.blockhash != job.source_blockhash {
        bail!(
            "source blockhash mismatch at slot {}: job={} canonical={}",
            job.source_slot,
            job.source_blockhash,
            canonical.blockhash
        );
    }

    let mut canonical_signatures = canonical.signatures;
    canonical_signatures.sort();

    let mut job_signatures = job.signatures.clone();
    job_signatures.sort();

    if canonical_signatures != job_signatures {
        bail!(
            "source signature set mismatch at slot {}: job_count={} canonical_count={}",
            job.source_slot,
            job_signatures.len(),
            canonical_signatures.len()
        );
    }

    let expected = execute_job(&job).context("verifier rejected job")?;

    if expected != miner_result {
        bail!(
            "miner result mismatch: expected {} but received {}",
            serde_json::to_string(&expected)?,
            serde_json::to_string(&miner_result)?
        );
    }

    println!(
        "VERIFIED job={} slot={} signatures={} commitment={}",
        expected.job_id, expected.source_slot, expected.item_count, expected.commitment_hex
    );

    Ok(())
}
