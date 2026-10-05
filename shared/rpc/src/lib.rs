use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

pub const DEFAULT_DEVNET_RPC: &str = "https://api.devnet.solana.com";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedBlock {
    pub slot: u64,
    pub blockhash: String,
    pub signatures: Vec<String>,
}

#[derive(Clone)]
pub struct SolanaRpcClient {
    url: String,
    http: Client,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct SignatureBlockResponse {
    blockhash: String,
    #[serde(default)]
    signatures: Vec<String>,
}

impl SolanaRpcClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            http: Client::new(),
        }
    }

    pub async fn latest_finalized_block(&self) -> Result<FinalizedBlock> {
        let highest_slot: u64 = self
            .rpc_call(
                "getSlot",
                json!([{
                    "commitment": "finalized"
                }]),
            )
            .await?;

        let start_slot = highest_slot.saturating_sub(128);
        let produced_slots: Vec<u64> = self
            .rpc_call(
                "getBlocks",
                json!([
                    start_slot,
                    highest_slot,
                    {
                        "commitment": "finalized"
                    }
                ]),
            )
            .await?;

        if produced_slots.is_empty() {
            bail!(
                "RPC returned no finalized produced blocks between slots {start_slot} and {highest_slot}"
            );
        }

        for slot in produced_slots.iter().rev().take(16) {
            if let Ok(block) = self.finalized_block(*slot).await {
                if !block.signatures.is_empty() {
                    return Ok(block);
                }
            }
        }

        let fallback_slot = *produced_slots
            .last()
            .context("produced block list unexpectedly empty")?;
        self.finalized_block(fallback_slot).await
    }

    pub async fn finalized_block(&self, slot: u64) -> Result<FinalizedBlock> {
        let block: SignatureBlockResponse = self
            .rpc_call(
                "getBlock",
                json!([
                    slot,
                    {
                        "commitment": "finalized",
                        "transactionDetails": "signatures",
                        "maxSupportedTransactionVersion": 0,
                        "rewards": false
                    }
                ]),
            )
            .await
            .with_context(|| format!("failed to fetch finalized block at slot {slot}"))?;

        Ok(FinalizedBlock {
            slot,
            blockhash: block.blockhash,
            signatures: block.signatures,
        })
    }

    async fn rpc_call<T>(&self, method: &str, params: Value) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self
            .http
            .post(&self.url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params
            }))
            .send()
            .await
            .with_context(|| format!("failed to send Solana RPC method {method}"))?
            .error_for_status()
            .with_context(|| format!("Solana RPC HTTP error for method {method}"))?
            .json::<RpcResponse<T>>()
            .await
            .with_context(|| format!("failed to decode Solana RPC response for method {method}"))?;

        if let Some(error) = response.error {
            bail!(
                "Solana RPC method {method} failed with code {}: {}",
                error.code,
                error.message
            );
        }

        response
            .result
            .ok_or_else(|| anyhow!("Solana RPC method {method} returned a null result"))
    }
}
