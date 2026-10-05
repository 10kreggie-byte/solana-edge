use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobSpec {
    pub version: u8,
    pub job_id: String,
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobResult {
    pub job_id: String,
    pub item_count: u64,
    pub commitment_hex: String,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("unsupported job version: {0}")]
    UnsupportedVersion(u8),
    #[error("job_id must not be empty")]
    EmptyJobId,
}

pub fn execute_job(job: &JobSpec) -> Result<JobResult, JobError> {
    if job.version != 0 {
        return Err(JobError::UnsupportedVersion(job.version));
    }
    if job.job_id.trim().is_empty() {
        return Err(JobError::EmptyJobId);
    }

    let mut signatures = job.signatures.clone();
    signatures.sort();

    let mut hasher = Sha256::new();

    for signature in &signatures {
        let bytes = signature.as_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }

    let commitment_hex = hex::encode(hasher.finalize());

    Ok(JobResult {
        job_id: job.job_id.clone(),
        item_count: signatures.len() as u64,
        commitment_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_is_order_independent() {
        let a = JobSpec {
            version: 0,
            job_id: "job-0001".into(),
            signatures: vec!["sig-b".into(), "sig-a".into()],
        };
        let b = JobSpec {
            version: 0,
            job_id: "job-0001".into(),
            signatures: vec!["sig-a".into(), "sig-b".into()],
        };

        assert_eq!(execute_job(&a).unwrap(), execute_job(&b).unwrap());
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let job = JobSpec {
            version: 1,
            job_id: "job-0002".into(),
            signatures: vec![],
        };

        assert!(matches!(
            execute_job(&job),
            Err(JobError::UnsupportedVersion(1))
        ));
    }
}
