use bs58;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_edge_miner_core::{JobResult, JobSpec};
use thiserror::Error;

const RECEIPT_DOMAIN: &[u8] = b"solana-edge/work-receipt/v0";
const RECEIPT_ID_DOMAIN: &[u8] = b"solana-edge/work-receipt-id/v0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletChallengeRequest {
    pub worker_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletChallenge {
    pub challenge_id: String,
    pub worker_pubkey: String,
    pub message: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerSessionRequest {
    pub challenge_id: String,
    pub worker_pubkey: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerSession {
    pub session_token: String,
    pub worker_pubkey: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkSubmission {
    pub session_token: String,
    pub worker_pubkey: String,
    pub job: JobSpec,
    pub result: JobResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkReceipt {
    pub version: u8,
    pub receipt_id: String,
    pub worker_pubkey: String,
    pub job_id: String,
    pub source_slot: u64,
    pub commitment_hex: String,
    pub item_count: u64,
    pub score: u64,
    pub reward_epoch: u64,
    pub issued_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedWorkReceipt {
    pub receipt: WorkReceipt,
    pub signer_pubkey: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinatorConfig {
    pub protocol_version: u8,
    pub cluster: String,
    pub receipt_signer_pubkey: String,
    pub session_ttl_seconds: u64,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid Solana public key")]
    InvalidPubkey,
    #[error("invalid signature encoding")]
    InvalidSignature,
    #[error("signature verification failed")]
    SignatureVerificationFailed,
    #[error("invalid receipt signer seed")]
    InvalidSignerSeed,
}

#[derive(Clone)]
pub struct ReceiptSigner {
    signing_key: SigningKey,
}

impl WorkReceipt {
    pub fn new(
        worker_pubkey: String,
        result: &JobResult,
        score: u64,
        reward_epoch: u64,
        issued_at_unix: i64,
    ) -> Self {
        let receipt_id = compute_receipt_id(
            &worker_pubkey,
            &result.job_id,
            result.source_slot,
            &result.commitment_hex,
        );

        Self {
            version: 0,
            receipt_id,
            worker_pubkey,
            job_id: result.job_id.clone(),
            source_slot: result.source_slot,
            commitment_hex: result.commitment_hex.clone(),
            item_count: result.item_count,
            score,
            reward_epoch,
            issued_at_unix,
        }
    }
}

impl ReceiptSigner {
    pub fn from_seed_hex(seed_hex: &str) -> Result<Self, ProtocolError> {
        let bytes = hex::decode(seed_hex).map_err(|_| ProtocolError::InvalidSignerSeed)?;
        let seed: [u8; 32] = bytes
            .try_into()
            .map_err(|_| ProtocolError::InvalidSignerSeed)?;

        Ok(Self {
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    pub fn verifying_key_base58(&self) -> String {
        bs58::encode(self.signing_key.verifying_key().as_bytes()).into_string()
    }

    pub fn sign(&self, receipt: WorkReceipt) -> SignedWorkReceipt {
        let message = receipt_signing_message(&receipt);
        let signature = self.signing_key.sign(&message);

        SignedWorkReceipt {
            receipt,
            signer_pubkey: self.verifying_key_base58(),
            signature_hex: hex::encode(signature.to_bytes()),
        }
    }
}

pub fn validate_solana_pubkey(pubkey: &str) -> Result<[u8; 32], ProtocolError> {
    let decoded = bs58::decode(pubkey)
        .into_vec()
        .map_err(|_| ProtocolError::InvalidPubkey)?;
    decoded
        .try_into()
        .map_err(|_| ProtocolError::InvalidPubkey)
}

pub fn verify_wallet_signature(
    worker_pubkey: &str,
    message: &[u8],
    signature_hex: &str,
) -> Result<(), ProtocolError> {
    let pubkey_bytes = validate_solana_pubkey(worker_pubkey)?;
    let verifying_key =
        VerifyingKey::from_bytes(&pubkey_bytes).map_err(|_| ProtocolError::InvalidPubkey)?;

    let signature_bytes =
        hex::decode(signature_hex).map_err(|_| ProtocolError::InvalidSignature)?;
    let signature =
        Signature::from_slice(&signature_bytes).map_err(|_| ProtocolError::InvalidSignature)?;

    verifying_key
        .verify(message, &signature)
        .map_err(|_| ProtocolError::SignatureVerificationFailed)
}

pub fn verify_signed_receipt(receipt: &SignedWorkReceipt) -> Result<(), ProtocolError> {
    let pubkey_bytes = validate_solana_pubkey(&receipt.signer_pubkey)?;
    let verifying_key =
        VerifyingKey::from_bytes(&pubkey_bytes).map_err(|_| ProtocolError::InvalidPubkey)?;

    let signature_bytes =
        hex::decode(&receipt.signature_hex).map_err(|_| ProtocolError::InvalidSignature)?;
    let signature =
        Signature::from_slice(&signature_bytes).map_err(|_| ProtocolError::InvalidSignature)?;

    verifying_key
        .verify(&receipt_signing_message(&receipt.receipt), &signature)
        .map_err(|_| ProtocolError::SignatureVerificationFailed)
}

fn compute_receipt_id(
    worker_pubkey: &str,
    job_id: &str,
    source_slot: u64,
    commitment_hex: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(RECEIPT_ID_DOMAIN);
    hash_len_prefixed(&mut hasher, worker_pubkey.as_bytes());
    hash_len_prefixed(&mut hasher, job_id.as_bytes());
    hasher.update(source_slot.to_le_bytes());
    hash_len_prefixed(&mut hasher, commitment_hex.as_bytes());
    hex::encode(hasher.finalize())
}

fn receipt_signing_message(receipt: &WorkReceipt) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(RECEIPT_DOMAIN);
    bytes.push(receipt.version);
    append_len_prefixed(&mut bytes, receipt.receipt_id.as_bytes());
    append_len_prefixed(&mut bytes, receipt.worker_pubkey.as_bytes());
    append_len_prefixed(&mut bytes, receipt.job_id.as_bytes());
    bytes.extend_from_slice(&receipt.source_slot.to_le_bytes());
    append_len_prefixed(&mut bytes, receipt.commitment_hex.as_bytes());
    bytes.extend_from_slice(&receipt.item_count.to_le_bytes());
    bytes.extend_from_slice(&receipt.score.to_le_bytes());
    bytes.extend_from_slice(&receipt.reward_epoch.to_le_bytes());
    bytes.extend_from_slice(&receipt.issued_at_unix.to_le_bytes());
    bytes
}

fn hash_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn append_len_prefixed(buffer: &mut Vec<u8>, value: &[u8]) {
    buffer.extend_from_slice(&(value.len() as u64).to_le_bytes());
    buffer.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_signature_round_trip() {
        let signer = ReceiptSigner::from_seed_hex(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .unwrap();

        let result = JobResult {
            job_id: "devnet-slot-1-test".into(),
            source_slot: 1,
            item_count: 7,
            commitment_hex: "abcd".into(),
        };

        let receipt = WorkReceipt::new(
            "11111111111111111111111111111111".into(),
            &result,
            7,
            100,
            1_700_000_000,
        );
        let signed = signer.sign(receipt);

        verify_signed_receipt(&signed).unwrap();
    }

    #[test]
    fn malformed_pubkey_is_rejected() {
        assert!(validate_solana_pubkey("not-a-solana-key").is_err());
    }
}
