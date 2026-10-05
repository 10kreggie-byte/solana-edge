use solana_edge_miner_core::{execute_job, JobSpec};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn execute_job_json(job_json: &str) -> Result<String, JsValue> {
    let job: JobSpec = serde_json::from_str(job_json)
        .map_err(|e| JsValue::from_str(&format!("invalid job JSON: {e}")))?;

    let result = execute_job(&job)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    serde_json::to_string(&result)
        .map_err(|e| JsValue::from_str(&format!("result serialization failed: {e}")))
}
