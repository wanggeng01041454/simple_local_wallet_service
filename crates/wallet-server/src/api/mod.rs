use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};
use wallet_core::{
    balance::TokenBalance,
    network::Network,
    notification::{format_locked_message, format_sign_message, SignEvent},
};

use crate::state::AppState;
use crate::util::parse_network;

// ---------------------------------------------------------------------------
// OpenAPI doc
// ---------------------------------------------------------------------------

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Simple Local Wallet Service API",
        description = "本地钱包服务的对外 API（端口 9293），提供多链钱包地址查询、余额查询和交易签名功能。\n\n支持的网络：solana、eth、bnb、arb、polygon\n\n**注意**：所有接口均要求钱包处于已解锁状态，否则返回 503 Service Unavailable。钱包的解锁/锁定操作通过管理端口 9292 进行。",
        version = "1.0.0",
    ),
    servers(
        (url = "http://127.0.0.1:9293", description = "本地 API 服务"),
    ),
    paths(get_address, get_balance, sign_transaction),
    components(schemas(
        Network,
        TokenBalance,
        AddressResponse,
        BalanceResponse,
        SignRequest,
        SignMetadata,
        SignResponse,
        ErrorResponse,
    ))
)]
pub struct ApiDoc;

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/wallet/address", get(get_address))
        .route("/api/wallet/balance", get(get_balance))
        .route("/api/wallet/sign", post(sign_transaction))
        .route("/api/docs/openapi.json", get(serve_openapi))
        .with_state(state)
}

async fn serve_openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

#[derive(Deserialize, IntoParams)]
struct NetworkQuery {
    /// 区块链网络标识：solana, eth, bnb, arb, polygon
    network: String,
}

#[derive(Serialize, ToSchema)]
struct AddressResponse {
    /// 区块链网络
    network: String,
    /// 钱包地址
    address: String,
}

#[derive(Serialize, ToSchema)]
struct BalanceResponse {
    /// 区块链网络
    network: String,
    /// 钱包地址
    address: String,
    /// 代币余额列表（原生代币 + USDT + USDC）
    balances: Vec<TokenBalance>,
}

#[derive(Deserialize, ToSchema)]
struct SignRequest {
    /// 区块链网络标识
    network: String,
    /// 待签名的交易数据（hex 编码，不含 0x 前缀）。EVM 链必须为 32 字节哈希（64 个 hex 字符）。
    transaction: String,
    /// 可选的签名元数据，用于 Telegram 通知
    metadata: Option<SignMetadata>,
}

#[derive(Deserialize, ToSchema)]
struct SignMetadata {
    /// 目标地址
    to: Option<String>,
    /// 转账金额描述
    value: Option<String>,
    /// 交易说明
    description: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct SignResponse {
    /// 区块链网络
    network: String,
    /// 签名结果（hex 编码）。EVM：130 hex = 65 字节；Solana：128 hex = 64 字节。
    signature: String,
    /// 签名时间（UTC）
    signed_at: String,
}

#[derive(Serialize, ToSchema)]
struct ErrorResponse {
    /// 错误标识
    error: String,
    /// 详细错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn locked_error() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "error": "wallet_locked",
            "message": "Wallet is locked. Please unlock via admin UI at http://localhost:9292"
        })),
    )
}

#[utoipa::path(
    get,
    path = "/api/wallet/address",
    params(NetworkQuery),
    responses(
        (status = 200, description = "成功返回钱包地址", body = AddressResponse),
        (status = 400, description = "无效的网络参数", body = ErrorResponse),
        (status = 404, description = "该网络尚未创建钱包", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "查询钱包地址",
    description = "根据网络名称返回对应的钱包地址。",
)]
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

#[utoipa::path(
    get,
    path = "/api/wallet/balance",
    params(NetworkQuery),
    responses(
        (status = 200, description = "成功返回余额列表", body = BalanceResponse),
        (status = 400, description = "无效的网络参数", body = ErrorResponse),
        (status = 404, description = "该网络尚未创建钱包", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "查询钱包余额",
    description = "查询指定网络钱包的链上真实余额，返回原生代币 + USDT + USDC 三种资产的余额。\n\n余额通过 JSON-RPC 实时查询链上数据。单个代币查询失败时不影响其他代币，失败的代币余额返回 \"0\"。",
)]
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

#[utoipa::path(
    post,
    path = "/api/wallet/sign",
    request_body = SignRequest,
    responses(
        (status = 200, description = "签名成功", body = SignResponse),
        (status = 400, description = "请求参数错误（无效网络名或无效 hex）", body = ErrorResponse),
        (status = 500, description = "签名过程出错", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "签名交易",
    description = "使用指定网络的私钥对交易数据进行签名。\n\n- **EVM 链**（eth/bnb/arb/polygon）：输入必须是 32 字节的交易哈希（64 个 hex 字符），返回 65 字节的 ECDSA 签名（r + s + v）。\n- **Solana**：输入为任意长度的交易数据（hex 编码），返回 64 字节的 Ed25519 签名。",
)]
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

    if matches!(
        network,
        Network::Eth | Network::Bnb | Network::Arb | Network::Polygon
    ) && tx_bytes.len() != 32
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "EVM tx must be a 32-byte hash"})),
        );
    }

    let private_key = match state.wallet.get_private_key_bytes(&network).await {
        Ok(k) => k,
        Err(wallet_core::wallet::WalletError::Locked) => return locked_error(),
        Err(wallet_core::wallet::WalletError::NotFound(_)) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "wallet not found for network"})),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    };

    let signature = match network {
        Network::Solana => wallet_core::solana_wallet::sign_message(&private_key, &tx_bytes),
        Network::Eth | Network::Bnb | Network::Arb | Network::Polygon => {
            sign_evm_message(&private_key, &tx_bytes)
        }
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

    async fn partially_unlocked_server() -> (TestServer, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let password = "test-password-123";
        let keys = wallet_core::evm_wallet::generate_keypair();
        state
            .wallet
            .save_wallet(&Network::Eth, &keys, password)
            .unwrap();
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
    async fn sign_evm_with_non_32byte_hash_returns_400() {
        let (server, _dir) = unlocked_server().await;
        // 16 bytes instead of 32
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "eth",
                "transaction": "ab".repeat(16)
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
    }

    #[tokio::test]
    async fn sign_returns_404_when_wallet_missing_for_network() {
        let (server, _dir) = partially_unlocked_server().await;
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "polygon",
                "transaction": "a".repeat(64)
            }))
            .await;
        assert_eq!(resp.status_code(), 404);
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

    #[tokio::test]
    async fn openapi_json_endpoint_returns_valid_spec() {
        let (server, _dir) = locked_server();
        let resp = server.get("/api/docs/openapi.json").await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["openapi"], "3.1.0");
        assert!(body["paths"]["/api/wallet/address"].is_object());
        assert!(body["paths"]["/api/wallet/balance"].is_object());
        assert!(body["paths"]["/api/wallet/sign"].is_object());
        assert!(body["components"]["schemas"]["Network"].is_object());
        assert!(body["components"]["schemas"]["TokenBalance"].is_object());
        assert!(body["components"]["schemas"]["SignRequest"].is_object());
    }
}
