use anyhow::{Context, Result};
use solana_edge_miner_core::{execute_job, JobSpec};
use std::{env, fs};

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .context("usage: solana-edge-miner-cli <job.json>")?;

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read job file: {path}"))?;
    let job: JobSpec =
        serde_json::from_str(&raw).with_context(|| format!("invalid JobSpec JSON: {path}"))?;

    let result = execute_job(&job).context("miner rejected job")?;

    eprintln!(
        "executed {} at slot {} ({} signatures)",
        result.job_id, result.source_slot, result.item_count
    );
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
