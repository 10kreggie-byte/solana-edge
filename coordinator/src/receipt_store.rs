use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use solana_edge_protocol::SignedWorkReceipt;
use std::sync::Mutex;

pub struct ReceiptStore {
    connection: Mutex<Connection>,
}

impl ReceiptStore {
    pub fn open(path: &str) -> Result<Self> {
        let connection =
            Connection::open(path).with_context(|| format!("failed to open receipt DB: {path}"))?;

        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS receipts (
                 receipt_id TEXT PRIMARY KEY,
                 worker_pubkey TEXT NOT NULL,
                 job_id TEXT NOT NULL,
                 source_slot INTEGER NOT NULL,
                 commitment_hex TEXT NOT NULL,
                 item_count INTEGER NOT NULL,
                 score INTEGER NOT NULL,
                 reward_epoch INTEGER NOT NULL,
                 issued_at_unix INTEGER NOT NULL,
                 signer_pubkey TEXT NOT NULL,
                 signature_hex TEXT NOT NULL,
                 signed_receipt_json TEXT NOT NULL,
                 UNIQUE(worker_pubkey, job_id)
             );
             CREATE INDEX IF NOT EXISTS receipts_epoch_worker_idx
                 ON receipts(reward_epoch, worker_pubkey);",
        )?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn insert(&self, signed: &SignedWorkReceipt) -> Result<bool> {
        let json = serde_json::to_string(signed)?;
        let receipt = &signed.receipt;

        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("receipt DB mutex poisoned"))?;

        let inserted = connection.execute(
            "INSERT OR IGNORE INTO receipts (
                receipt_id,
                worker_pubkey,
                job_id,
                source_slot,
                commitment_hex,
                item_count,
                score,
                reward_epoch,
                issued_at_unix,
                signer_pubkey,
                signature_hex,
                signed_receipt_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                receipt.receipt_id,
                receipt.worker_pubkey,
                receipt.job_id,
                receipt.source_slot,
                receipt.commitment_hex,
                receipt.item_count,
                receipt.score,
                receipt.reward_epoch,
                receipt.issued_at_unix,
                signed.signer_pubkey,
                signed.signature_hex,
                json,
            ],
        )?;

        Ok(inserted == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_edge_miner_core::JobResult;
    use solana_edge_protocol::{ReceiptSigner, WorkReceipt};

    #[test]
    fn duplicate_wallet_job_is_rejected_across_receipt_ids() {
        let store = ReceiptStore::open(":memory:").unwrap();
        let signer = ReceiptSigner::from_seed_hex(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .unwrap();

        let result = JobResult {
            job_id: "job-a".into(),
            source_slot: 5,
            item_count: 4,
            commitment_hex: "abcd".into(),
        };

        let first = signer.sign(WorkReceipt::new(
            "11111111111111111111111111111111".into(),
            &result,
            4,
            10,
            100,
        ));

        let second = signer.sign(WorkReceipt::new(
            "11111111111111111111111111111111".into(),
            &result,
            4,
            10,
            101,
        ));

        assert!(store.insert(&first).unwrap());
        assert!(!store.insert(&second).unwrap());
    }
}
