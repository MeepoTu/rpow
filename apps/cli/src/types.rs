use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AuthRequestResponse {
    pub ok: bool,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct MeResponse {
    pub email: String,
    pub balance: i64,
    pub minted: i64,
    pub sent: i64,
    pub received: i64,
}

#[derive(Debug, Deserialize)]
pub struct ChallengeResponse {
    pub challenge_id: String,
    pub nonce_prefix: String,
    pub difficulty_bits: u32,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct MintRequestBody {
    pub challenge_id: String,
    pub solution_nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct MintResponse {
    pub token: TokenSummary,
}

#[derive(Debug, Deserialize)]
pub struct TokenSummary {
    pub id: String,
    pub value: i64,
    pub issued_at: String,
}

#[derive(Debug, Serialize)]
pub struct SendRequestBody {
    pub recipient_email: String,
    pub amount: i64,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct SendResponse {
    pub ok: bool,
    pub transferred: i64,
    pub recipient_email: String,
    pub transfer_id: String,
    pub pending: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityEntry {
    pub r#type: String,
    pub amount: i64,
    pub counterparty_email: Option<String>,
    pub at: String,
}

#[derive(Debug, Deserialize)]
pub struct LedgerResponse {
    pub total_minted: i64,
    pub total_transferred: i64,
    pub circulating_supply: i64,
    pub current_difficulty_bits: i64,
    pub user_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub retry_after: Option<u64>,
}
