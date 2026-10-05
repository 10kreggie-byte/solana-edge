use anyhow::{Context, Result};
use solana_edge_settlement::build_manifest_from_sqlite;
use std::env;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let database_path = args
        .next()
        .context("usage: solana-edge-settlement <receipts.sqlite> <reward_epoch>")?;
    let reward_epoch = args
        .next()
        .context("usage: solana-edge-settlement <receipts.sqlite> <reward_epoch>")?
        .parse::<u64>()
        .context("reward_epoch must be a u64")?;

    let manifest = build_manifest_from_sqlite(&database_path, reward_epoch)?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}
