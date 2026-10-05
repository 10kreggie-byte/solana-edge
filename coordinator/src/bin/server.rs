use anyhow::{bail, Context, Result};
use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use solana_edge_coordinator::{create_job, receipt_store::ReceiptStore};
use solana_edge_protocol::{
    validate_solana_pubkey, verify_wallet_signature, CoordinatorConfig, ReceiptSigner,
    SignedWorkReceipt, WalletChallenge, WalletChallengeRequest, WorkReceipt, WorkSubmission,
    WorkerSession, WorkerSessionRequest,
};
use solana_edge_rpc::{SolanaRpcClient, DEFAULT_DEVNET_RPC};
use solana_edge_verifier::verify_job_result;
use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

const CHALLENGE_TTL_SECONDS: i64 = 300;
const SESSION_TTL_SECONDS: i64 = 1_800;

#[derive(Clone)]
struct AppState {
    rpc: SolanaRpcClient,
    signer: Arc<ReceiptSigner>,
    challenges: Arc<Mutex<HashMap<String, StoredChallenge>>>,
    sessions: Arc<Mutex<HashMap<String, StoredSession>>>,
    receipt_store: Arc<ReceiptStore>,
}

#[derive(Clone)]
struct StoredChallenge {
    worker_pubkey: String,
    message: String,
    expires_at_unix: i64,
}

#[derive(Clone)]
struct StoredSession {
    worker_pubkey: String,
    expires_at_unix: i64,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    cluster: &'static str,
}

type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn upstream(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cluster = env::var("SOLANA_EDGE_CLUSTER").unwrap_or_else(|_| "devnet".to_string());
    if cluster != "devnet" {
        bail!("MVP safety guard: SOLANA_EDGE_CLUSTER must be devnet");
    }

    let signing_seed = env::var("SOLANA_EDGE_RECEIPT_SIGNING_KEY_HEX")
        .context("SOLANA_EDGE_RECEIPT_SIGNING_KEY_HEX must be a 32-byte hex seed")?;
    let signer = ReceiptSigner::from_seed_hex(&signing_seed)
        .map_err(|error| anyhow::anyhow!("invalid receipt signing key: {error}"))?;

    let rpc_url = env::var("SOLANA_RPC_URL").unwrap_or_else(|_| DEFAULT_DEVNET_RPC.to_string());
    let rpc = SolanaRpcClient::new(rpc_url);

    let allowed_origin = env::var("SOLANA_EDGE_WEB_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_string())
        .parse::<HeaderValue>()
        .context("invalid SOLANA_EDGE_WEB_ORIGIN")?;

    let cors = CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([CONTENT_TYPE]);

    let receipt_db_path = env::var("SOLANA_EDGE_RECEIPT_DB")
        .unwrap_or_else(|_| "solana-edge-receipts.sqlite3".to_string());
    let receipt_store = ReceiptStore::open(&receipt_db_path)?;

    let state = AppState {
        rpc,
        signer: Arc::new(signer),
        challenges: Arc::new(Mutex::new(HashMap::new())),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        receipt_store: Arc::new(receipt_store),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/config", get(config))
        .route("/v1/job", get(job))
        .route("/v1/challenge", post(challenge))
        .route("/v1/session", post(session))
        .route("/v1/submit", post(submit))
        .layer(cors)
        .with_state(state);

    let bind = env::var("SOLANA_EDGE_BIND").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let address: SocketAddr = bind.parse().context("invalid SOLANA_EDGE_BIND")?;
    let listener = tokio::net::TcpListener::bind(address).await?;

    eprintln!("Solana Edge coordinator API listening on http://{address}");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        cluster: "devnet",
    })
}

async fn config(State(state): State<AppState>) -> Json<CoordinatorConfig> {
    Json(CoordinatorConfig {
        protocol_version: 0,
        cluster: "devnet".into(),
        receipt_signer_pubkey: state.signer.verifying_key_base58(),
        session_ttl_seconds: SESSION_TTL_SECONDS as u64,
    })
}

async fn job(State(state): State<AppState>) -> ApiResult<solana_edge_miner_core::JobSpec> {
    create_job(&state.rpc, "devnet", None)
        .await
        .map(Json)
        .map_err(|error| ApiError::upstream(format!("failed to create finalized job: {error}")))
}

async fn challenge(
    State(state): State<AppState>,
    Json(request): Json<WalletChallengeRequest>,
) -> ApiResult<WalletChallenge> {
    validate_solana_pubkey(&request.worker_pubkey)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let now = now_unix();
    let expires_at_unix = now + CHALLENGE_TTL_SECONDS;
    let challenge_id = Uuid::new_v4().to_string();
    let message = format!(
        "Solana Edge Devnet worker authentication\nWallet: {}\nChallenge: {}\nExpires: {}\nNo transaction will be sent.",
        request.worker_pubkey, challenge_id, expires_at_unix
    );

    state.challenges.lock().await.insert(
        challenge_id.clone(),
        StoredChallenge {
            worker_pubkey: request.worker_pubkey.clone(),
            message: message.clone(),
            expires_at_unix,
        },
    );

    Ok(Json(WalletChallenge {
        challenge_id,
        worker_pubkey: request.worker_pubkey,
        message,
        expires_at_unix,
    }))
}

async fn session(
    State(state): State<AppState>,
    Json(request): Json<WorkerSessionRequest>,
) -> ApiResult<WorkerSession> {
    let stored = state
        .challenges
        .lock()
        .await
        .remove(&request.challenge_id)
        .ok_or_else(|| ApiError::unauthorized("unknown or already-used challenge"))?;

    if stored.expires_at_unix < now_unix() {
        return Err(ApiError::unauthorized("challenge expired"));
    }
    if stored.worker_pubkey != request.worker_pubkey {
        return Err(ApiError::unauthorized("challenge wallet mismatch"));
    }

    verify_wallet_signature(
        &request.worker_pubkey,
        stored.message.as_bytes(),
        &request.signature_hex,
    )
    .map_err(|error| ApiError::unauthorized(format!("wallet signature rejected: {error}")))?;

    let session_token = Uuid::new_v4().to_string();
    let expires_at_unix = now_unix() + SESSION_TTL_SECONDS;

    state.sessions.lock().await.insert(
        session_token.clone(),
        StoredSession {
            worker_pubkey: request.worker_pubkey.clone(),
            expires_at_unix,
        },
    );

    Ok(Json(WorkerSession {
        session_token,
        worker_pubkey: request.worker_pubkey,
        expires_at_unix,
    }))
}

async fn submit(
    State(state): State<AppState>,
    Json(submission): Json<WorkSubmission>,
) -> ApiResult<SignedWorkReceipt> {
    let stored_session = state
        .sessions
        .lock()
        .await
        .get(&submission.session_token)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("unknown worker session"))?;

    if stored_session.expires_at_unix < now_unix() {
        return Err(ApiError::unauthorized("worker session expired"));
    }
    if stored_session.worker_pubkey != submission.worker_pubkey {
        return Err(ApiError::unauthorized("worker session wallet mismatch"));
    }

    let verified = verify_job_result(&state.rpc, &submission.job, &submission.result)
        .await
        .map_err(|error| ApiError::bad_request(format!("work verification failed: {error}")))?;

    let issued_at_unix = now_unix();
    let reward_epoch = (issued_at_unix.max(0) as u64) / 3_600;
    let score = verified.item_count;

    let receipt = WorkReceipt::new(
        submission.worker_pubkey,
        &verified,
        score,
        reward_epoch,
        issued_at_unix,
    );

    let signed = state.signer.sign(receipt);
    let inserted = state
        .receipt_store
        .insert(&signed)
        .map_err(|error| ApiError::upstream(format!("receipt DB write failed: {error}")))?;

    if !inserted {
        return Err(ApiError::conflict(
            "receipt already issued for this wallet/job",
        ));
    }

    Ok(Json(signed))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs() as i64
}
