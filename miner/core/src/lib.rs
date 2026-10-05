use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const JOB_DOMAIN: &[u8] = b"solana-edge/job/v0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobSpec {
    pub version: u8,
    pub job_id: String,
    pub source_cluster: String,
    pub source_slot: u64,
    pub source_blockhash: String,
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobResult {
    pub job_id: String,
    pub source_slot: u64,
    pub item_count: u64,
    pub commitment_hex: String,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("unsupported job version: {0}")]
    UnsupportedVersion(u8),
    #[error("job_id must not be empty")]
    EmptyJobId,
    #[error("source_cluster must not be empty")]
    EmptyCluster,
    #[error("source_blockhash must not be empty")]
    EmptyBlockhash,
}

fn hash_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

pub fn execute_job(job: &JobSpec) -> Result<JobResult, JobError> {
    if job.version != 0 {
        return Err(JobError::UnsupportedVersion(job.version));
    }
    if job.job_id.trim().is_empty() {
        return Err(JobError::EmptyJobId);
    }
    if job.source_cluster.trim().is_empty() {
        return Err(JobError::EmptyCluster);
    }
    if job.source_blockhash.trim().is_empty() {
        return Err(JobError::EmptyBlockhash);
    }

    let mut signatures = job.signatures.clone();
    signatures.sort();

    let mut hasher = Sha256::new();
    hasher.update(JOB_DOMAIN);
    hasher.update([job.version]);
    hash_len_prefixed(&mut hasher, job.job_id.as_bytes());
    hash_len_prefixed(&mut hasher, job.source_cluster.as_bytes());
    hasher.update(job.source_slot.to_le_bytes());
    hash_len_prefixed(&mut hasher, job.source_blockhash.as_bytes());
    hasher.update((signatures.len() as u64).to_le_bytes());

    for signature in &signatures {
        hash_len_prefixed(&mut hasher, signature.as_bytes());
    }

    Ok(JobResult {
        job_id: job.job_id.clone(),
        source_slot: job.source_slot,
        item_count: signatures.len() as u64,
        commitment_hex: hex::encode(hasher.finalize()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(signatures: Vec<&str>) -> JobSpec {
        JobSpec {
            version: 0,
            job_id: "job-0001".into(),
            source_cluster: "devnet".into(),
            source_slot: 123,
            source_blockhash: "blockhash-a".into(),
            signatures: signatures.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn result_is_order_independent() {
        let a = job(vec!["sig-b", "sig-a"]);
        let b = job(vec!["sig-a", "sig-b"]);

        assert_eq!(execute_job(&a).unwrap(), execute_job(&b).unwrap());
    }

    #[test]
    fn commitment_is_bound_to_source_block() {
        let a = job(vec!["sig-a"]);
        let mut b = a.clone();
        b.source_blockhash = "blockhash-b".into();

        assert_ne!(
            execute_job(&a).unwrap().commitment_hex,
            execute_job(&b).unwrap().commitment_hex
        );
    }

    #[test]
    fn commitment_is_bound_to_job_id() {
        let a = job(vec!["sig-a"]);
        let mut b = a.clone();
        b.job_id = "job-0002".into();

        assert_ne!(
            execute_job(&a).unwrap().commitment_hex,
            execute_job(&b).unwrap().commitment_hex
        );
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let mut input = job(vec![]);
        input.version = 1;

        assert!(matches!(
            execute_job(&input),
            Err(JobError::UnsupportedVersion(1))
        ));
    }
}
