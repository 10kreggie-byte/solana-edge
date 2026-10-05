use anyhow::{bail, Context, Result};
use solana_edge_miner_core::{execute_job, JobResult, JobSpec};
use solana_edge_rpc::SolanaRpcClient;

pub async fn verify_job_result(
    rpc: &SolanaRpcClient,
    job: &JobSpec,
    miner_result: &JobResult,
) -> Result<JobResult> {
    if job.source_cluster != "devnet" {
        bail!(
            "MVP safety guard: verifier only accepts devnet jobs, received {}",
            job.source_cluster
        );
    }

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

    let expected = execute_job(job).context("verifier rejected job")?;

    if expected != *miner_result {
        bail!(
            "miner result mismatch: expected {} but received {}",
            serde_json::to_string(&expected)?,
            serde_json::to_string(miner_result)?
        );
    }

    Ok(expected)
}
