use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use wallet_core::{
    network::Network,
    notification::{format_locked_message, format_sign_message, SignEvent},
};

use crate::state::AppState;
use crate::util::parse_network;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/wallet/address", get(get_address))
        .route("/api/wallet/balance", get(get_balance))
        .route("/api/wallet/sign", post(sign_transaction))
        .with_state(state)
}

#[derive(Deserialize)]
struct NetworkQuery {
    network: String,
}

fn locked_error() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "error": "wallet_locked",
            "message": "Wallet is locked. Please unlock via admin UI at http://localhost:9292"
        })),
    )
}

async fn get_address(
    State(state): State<AppState>,
    Query(params): Query<NetworkQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let network = match parse_network(&params.network) {
        Some(n) => n,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_network"})),
            )
        }
    };

    match state.wallet.get_address(&network).await {
        Ok(address) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "network": params.network,
                "address": address
            })),
        ),
        Err(wallet_core::wallet::WalletError::Locked) => {
            state.send_telegram(&format_locked_message("address")).await;
            locked_error()
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn get_balance(
    State(state): State<AppState>,
    Query(params): Query<NetworkQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let network = match parse_network(&params.network) {
        Some(n) => n,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_network"})),
            )
        }
    };

    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("balance")).await;
        return locked_error();
    }

    let address = match state.wallet.get_address(&network).await {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    };

    let rpc_url = {
        let config = state.config.read().await;
        config.rpc.url_for(&network).to_string()
    };

    let balances = wallet_core::balance::query_balances(&rpc_url, &network, &address).await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "network": params.network,
            "address": address,
            "balances": balances
        })),
    )
}

#[derive(Deserialize)]
struct SignRequest {
    network: String,
    transaction: String,
    metadata: Option<SignMetadata>,
}

#[derive(Deserialize)]
struct SignMetadata {
    to: Option<String>,
    value: Option<String>,
    description: Option<String>,
}

async fn sign_transaction(
    State(state): State<AppState>,
    Json(req): Json<SignRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let network = match parse_network(&req.network) {
        Some(n) => n,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_network"})),
            )
        }
    };

    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign")).await;
        return locked_error();
    }

    let tx_bytes = match hex::decode(&req.transaction) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid transaction hex"})),
            )
        }
    };

    let private_key = match state.wallet.get_private_key_bytes(&network).await {
        Ok(k) => k,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    };

    let signature = match network {
        Network::Solana => wallet_core::solana_wallet::sign_message(&private_key, &tx_bytes),
        Network::Eth | Network::Bnb | Network::Arb | Network::Polygon => sign_evm_message(&private_key, &tx_bytes),
    };

    let signature = match signature {
        Ok(s) => hex::encode(s),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    };

    let signed_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

    let meta = req.metadata.as_ref();
    let event = SignEvent {
        network: network.display_name().to_string(),
        to: meta.and_then(|m| m.to.clone()),
        value: meta.and_then(|m| m.value.clone()),
        description: meta.and_then(|m| m.description.clone()),
        signed_at: signed_at.clone(),
    };
    state.send_telegram(&format_sign_message(&event)).await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "network": req.network,
            "signature": signature,
            "signed_at": signed_at
        })),
    )
}

fn sign_evm_message(
    private_key_bytes: &[u8],
    message: &[u8],
) -> Result<Vec<u8>, wallet_core::wallet::WalletError> {
    use alloy::primitives::B256;
    use alloy::signers::local::PrivateKeySigner;
    use alloy::signers::SignerSync;

    if private_key_bytes.len() != 32 {
        return Err(wallet_core::wallet::WalletError::NotFound(
            "invalid key length".to_string(),
        ));
    }
    let key = B256::from_slice(private_key_bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| wallet_core::wallet::WalletError::NotFound(e.to_string()))?;

    if message.len() != 32 {
        return Err(wallet_core::wallet::WalletError::NotFound(
            "EVM tx must be a 32-byte hash".to_string(),
        ));
    }
    let hash = B256::from_slice(message);
    let sig = signer
        .sign_hash_sync(&hash)
        .map_err(|e| wallet_core::wallet::WalletError::NotFound(e.to_string()))?;
    Ok(sig.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use tempfile::TempDir;

    fn locked_server() -> (TestServer, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let app = router(state);
        (TestServer::new(app).unwrap(), dir)
    }

    #[tokio::test]
    async fn get_address_when_locked_returns_503() {
        let (server, _dir) = locked_server();
        let resp = server
            .get("/api/wallet/address")
            .add_query_param("network", "eth")
            .await;
        assert_eq!(resp.status_code(), 503);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "wallet_locked");
    }

    #[tokio::test]
    async fn get_balance_when_locked_returns_503() {
        let (server, _dir) = locked_server();
        let resp = server
            .get("/api/wallet/balance")
            .add_query_param("network", "eth")
            .await;
        assert_eq!(resp.status_code(), 503);
    }

    #[tokio::test]
    async fn post_sign_when_locked_returns_503() {
        let (server, _dir) = locked_server();
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "eth",
                "transaction": "deadbeef"
            }))
            .await;
        assert_eq!(resp.status_code(), 503);
    }

    #[tokio::test]
    async fn get_address_with_invalid_network_returns_400_or_503() {
        let (server, _dir) = locked_server();
        let resp = server
            .get("/api/wallet/address")
            .add_query_param("network", "invalid_network")
            .await;
        assert!(resp.status_code() == 503 || resp.status_code() == 400);
    }

    /// Create wallets for all networks, unlock, return a ready-to-use server.
    async fn unlocked_server() -> (TestServer, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let password = "test-password-123";

        // Generate a wallet for every supported network
        for network in wallet_core::network::Network::all() {
            let keys = match network {
                Network::Solana => wallet_core::solana_wallet::generate_keypair(),
                _ => wallet_core::evm_wallet::generate_keypair(),
            };
            state.wallet.save_wallet(network, &keys, password).unwrap();
        }
        state.wallet.unlock(password).await.unwrap();

        let app = router(state);
        (TestServer::new(app).unwrap(), dir)
    }

    #[tokio::test]
    async fn sign_eth_returns_valid_signature() {
        let (server, _dir) = unlocked_server().await;
        let tx_hash = "a".repeat(64); // 32 bytes in hex
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "eth",
                "transaction": tx_hash
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["network"], "eth");
        let sig = body["signature"].as_str().unwrap();
        // EVM signature is 65 bytes (r=32 + s=32 + v=1) = 130 hex chars
        assert_eq!(sig.len(), 130);
        assert!(body["signed_at"].as_str().is_some());
    }

    #[tokio::test]
    async fn sign_bnb_returns_valid_signature() {
        let (server, _dir) = unlocked_server().await;
        let tx_hash = "b".repeat(64);
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "bnb",
                "transaction": tx_hash
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["network"], "bnb");
        assert_eq!(body["signature"].as_str().unwrap().len(), 130);
    }

    #[tokio::test]
    async fn sign_arb_returns_valid_signature() {
        let (server, _dir) = unlocked_server().await;
        let tx_hash = "c".repeat(64);
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "arb",
                "transaction": tx_hash
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["network"], "arb");
        assert_eq!(body["signature"].as_str().unwrap().len(), 130);
    }

    #[tokio::test]
    async fn sign_polygon_returns_valid_signature() {
        let (server, _dir) = unlocked_server().await;
        let tx_hash = "d".repeat(64);
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "polygon",
                "transaction": tx_hash
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["network"], "polygon");
        assert_eq!(body["signature"].as_str().unwrap().len(), 130);
    }

    #[tokio::test]
    async fn sign_solana_returns_valid_signature() {
        let (server, _dir) = unlocked_server().await;
        let tx_hash = "e".repeat(64);
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "solana",
                "transaction": tx_hash
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["network"], "solana");
        // Ed25519 signature is 64 bytes = 128 hex chars
        assert_eq!(body["signature"].as_str().unwrap().len(), 128);
    }

    #[tokio::test]
    async fn sign_with_invalid_network_returns_400() {
        let (server, _dir) = unlocked_server().await;
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "invalid",
                "transaction": "a".repeat(64)
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_network");
    }

    #[tokio::test]
    async fn sign_evm_with_non_32byte_hash_returns_500() {
        let (server, _dir) = unlocked_server().await;
        // 16 bytes instead of 32
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "eth",
                "transaction": "ab".repeat(16)
            }))
            .await;
        assert_eq!(resp.status_code(), 500);
    }

    #[tokio::test]
    async fn sign_with_invalid_hex_returns_400() {
        let (server, _dir) = unlocked_server().await;
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "eth",
                "transaction": "not-valid-hex"
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid transaction hex");
    }
}
