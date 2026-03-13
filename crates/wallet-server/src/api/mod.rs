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
    notification::format_locked_message,
};

use base64::Engine as _;
use crate::extractor::ValidatedJson;
use crate::state::AppState;
use crate::util::parse_network;

// ---------------------------------------------------------------------------
// OpenAPI doc
// ---------------------------------------------------------------------------

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Simple Local Wallet Service API",
        description = "本地钱包服务的对外 API（端口 9293），提供多链钱包地址查询、余额查询和交易签名功能。\n\n支持的网络：solana、eth、bnb、arb、polygon\n\n**注意**：所有签名接口均要求 request_id 字段（必填）和钱包处于已解锁状态。解析错误响应不含 request_id。",
        version = "2.0.0",
    ),
    servers(
        (url = "http://127.0.0.1:9293", description = "本地 API 服务"),
    ),
    paths(get_address, get_balance, sign_solana, sign_evm_transaction, sign_evm_typed_data),
    components(schemas(
        Network,
        TokenBalance,
        AddressResponse,
        BalanceResponse,
        SignSolanaRequest,
        SignSolanaResponse,
        SignEvmTransactionRequest,
        SignEvmTransactionResponse,
        SignEvmTypedDataRequest,
        SignEvmTypedDataResponse,
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
        .route("/api/wallet/sign/solana", post(sign_solana))
        .route("/api/wallet/sign/evm/transaction", post(sign_evm_transaction))
        .route("/api/wallet/sign/evm/typed-data", post(sign_evm_typed_data))
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

#[derive(Serialize, ToSchema)]
struct ErrorResponse {
    /// 错误标识
    error: String,
    /// 详细错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    /// 透传自请求的唯一标识
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

// --- Solana ---

#[derive(Deserialize, ToSchema)]
struct SignSolanaRequest {
    /// 请求唯一标识，原样透传至响应；必填
    #[schema(required = true, value_type = String)]
    #[serde(default)]
    request_id: Option<String>,
    /// 输入交易的编码格式：`"base64"` 或 `"base58"`
    encoding: String,
    /// 完整 Solana 交易（已含 recent_blockhash），由 encoding 字段指定编码
    transaction: String,
}

#[derive(Serialize, ToSchema)]
struct SignSolanaResponse {
    /// 透传自请求的唯一标识
    request_id: String,
    /// 签名后的完整 Solana 交易（固定 base58 编码）
    transaction: String,
}

// --- EVM Transaction ---

#[derive(Deserialize, ToSchema)]
struct SignEvmTransactionRequest {
    /// 请求唯一标识，原样透传至响应；必填
    #[schema(required = true, value_type = String)]
    #[serde(default)]
    request_id: Option<String>,
    /// 目标网络：eth | bnb | arb | polygon
    network: String,
    /// 未签名的完整 EVM 交易，RLP 编码转 hex，含 0x 前缀，含零签名字段
    transaction: String,
}

#[derive(Serialize, ToSchema)]
struct SignEvmTransactionResponse {
    /// 透传自请求的唯一标识
    request_id: String,
    /// 透传自请求的网络标识
    network: String,
    /// 签名后的完整 EVM 交易，RLP 编码转 hex，含 0x 前缀
    transaction: String,
}

// --- EIP-712 ---

#[derive(Deserialize, ToSchema)]
struct SignEvmTypedDataRequest {
    /// 请求唯一标识，原样透传至响应；必填
    #[schema(required = true, value_type = String)]
    #[serde(default)]
    request_id: Option<String>,
    /// 目标网络：eth | bnb | arb | polygon
    network: String,
    /// 标准 EIP-712 结构，对齐 eth_signTypedData_v4，含 domain / types / primaryType / message 四个字段
    #[schema(value_type = Object)]
    typed_data: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
struct SignEvmTypedDataResponse {
    /// 透传自请求的唯一标识
    request_id: String,
    /// 透传自请求的网络标识
    network: String,
    /// 完整 ECDSA 签名（65 字节，hex + 0x 前缀，共 132 字符）
    signature: String,
    /// 签名 r 分量（32 字节，hex + 0x 前缀，共 66 字符）
    r: String,
    /// 签名 s 分量（32 字节，hex + 0x 前缀，共 66 字符）
    s: String,
    /// 签名 v 分量（1 字节，hex + 0x 前缀，如 "0x1b" 或 "0x1c"）
    v: String,
}

// ---------------------------------------------------------------------------
// Helpers
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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

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
    path = "/api/wallet/sign/solana",
    request_body = SignSolanaRequest,
    responses(
        (status = 200, description = "签名成功", body = SignSolanaResponse),
        (status = 400, description = "请求参数错误（含 missing_request_id / invalid_encoding / invalid_transaction / already_signed / signer_not_required）", body = ErrorResponse),
        (status = 404, description = "该网络尚未创建钱包", body = ErrorResponse),
        (status = 415, description = "Content-Type 不是 application/json，响应不含 request_id", body = ErrorResponse),
        (status = 422, description = "JSON 结构正确但字段类型不匹配，响应不含 request_id", body = ErrorResponse),
        (status = 500, description = "内部签名错误", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "Solana 交易签名",
    description = "对 Solana 交易进行签名，支持 base64 和 base58 编码输入，返回 base58 编码的已签名交易。\n\n注意：415 和 422 错误发生在请求解析阶段，响应中不含 request_id，这是预期行为。",
)]
async fn sign_solana(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SignSolanaRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let rid = match req.request_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing_request_id"})),
            )
        }
    };
    let rid = rid.as_str();

    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign/solana")).await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "wallet_locked",
                "message": "Wallet is locked. Please unlock via admin UI at http://localhost:9292",
                "request_id": rid
            })),
        );
    }

    let tx_bytes = match req.encoding.as_str() {
        "base64" => {
            match base64::engine::general_purpose::STANDARD.decode(&req.transaction) {
                Ok(b) => b,
                Err(_) => return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid_transaction", "message": "base64 decode failed", "request_id": rid})),
                ),
            }
        }
        "base58" => {
            match bs58::decode(&req.transaction).into_vec() {
                Ok(b) => b,
                Err(_) => return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid_transaction", "message": "base58 decode failed", "request_id": rid})),
                ),
            }
        }
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_encoding", "message": "encoding must be base64 or base58", "request_id": rid})),
        ),
    };

    let private_key = match state
        .wallet
        .get_private_key_bytes(&wallet_core::Network::Solana)
        .await
    {
        Ok(k) => k,
        Err(wallet_core::wallet::WalletError::Locked) => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})),
        ),
        Err(wallet_core::wallet::WalletError::NotFound(_)) => return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "wallet_not_found", "request_id": rid})),
        ),
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
        ),
    };

    let (signed_bytes, tx_id) =
        match wallet_core::solana_wallet::sign_transaction(&private_key, &tx_bytes) {
            Ok(r) => r,
            Err(wallet_core::wallet::WalletError::SignerNotRequired) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "signer_not_required", "request_id": rid})),
            ),
            Err(wallet_core::wallet::WalletError::AlreadySigned) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "already_signed", "request_id": rid})),
            ),
            Err(wallet_core::wallet::WalletError::InvalidTransaction(msg)) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_transaction", "message": msg, "request_id": rid})),
            ),
            Err(e) => return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
            ),
        };

    let signed_tx_b58 = bs58::encode(&signed_bytes).into_string();

    let signed_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let event = wallet_core::notification::SignEvent::Solana {
        tx_id,
        transaction: signed_tx_b58.clone(),
        signed_at,
    };
    state
        .send_telegram(&wallet_core::notification::format_sign_message(&event))
        .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "request_id": rid,
            "transaction": signed_tx_b58,
        })),
    )
}

#[utoipa::path(
    post,
    path = "/api/wallet/sign/evm/transaction",
    request_body = SignEvmTransactionRequest,
    responses(
        (status = 200, description = "签名成功", body = SignEvmTransactionResponse),
        (status = 400, description = "请求参数错误（含 missing_request_id / invalid_network / invalid_transaction / already_signed / chain_id_mismatch / unsupported_tx_type）", body = ErrorResponse),
        (status = 404, description = "该网络尚未创建钱包", body = ErrorResponse),
        (status = 415, description = "Content-Type 不是 application/json，响应不含 request_id", body = ErrorResponse),
        (status = 422, description = "JSON 结构正确但字段类型不匹配，响应不含 request_id", body = ErrorResponse),
        (status = 500, description = "内部签名错误", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "EVM 交易签名",
    description = "对 EVM 交易（Legacy/EIP-2930/EIP-1559）进行签名，返回 hex 编码的已签名交易。\n\n注意：415 和 422 错误发生在请求解析阶段，响应中不含 request_id，这是预期行为。",
)]
async fn sign_evm_transaction(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SignEvmTransactionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let rid = match req.request_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing_request_id"})),
        ),
    };
    let rid = rid.as_str();

    let network = match crate::util::parse_network(&req.network) {
        Some(n) if n != wallet_core::Network::Solana => n,
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_network", "request_id": rid})),
        ),
    };

    let chain_id = network.chain_id().unwrap();

    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign/evm/transaction")).await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})),
        );
    }

    let tx_str = req.transaction.trim_start_matches("0x");
    let tx_bytes = match hex::decode(tx_str) {
        Ok(b) => b,
        Err(_) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_transaction", "message": "hex decode failed", "request_id": rid})),
        ),
    };

    let private_key = match state.wallet.get_private_key_bytes(&network).await {
        Ok(k) => k,
        Err(wallet_core::wallet::WalletError::Locked) => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})),
        ),
        Err(wallet_core::wallet::WalletError::NotFound(_)) => return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "wallet_not_found", "request_id": rid})),
        ),
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
        ),
    };

    let (signed_bytes, from, to, value) =
        match wallet_core::evm_wallet::sign_transaction(&private_key, &tx_bytes, chain_id) {
            Ok(r) => r,
            Err(wallet_core::wallet::WalletError::AlreadySigned) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "already_signed", "request_id": rid})),
            ),
            Err(wallet_core::wallet::WalletError::ChainIdMismatch { expected, actual }) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "chain_id_mismatch",
                    "message": format!("expected {expected}, got {actual}"),
                    "request_id": rid
                })),
            ),
            Err(wallet_core::wallet::WalletError::UnsupportedTxType(t)) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "unsupported_tx_type",
                    "message": format!("type {t} not supported"),
                    "request_id": rid
                })),
            ),
            Err(wallet_core::wallet::WalletError::InvalidTransaction(msg)) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_transaction", "message": msg, "request_id": rid})),
            ),
            Err(e) => return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
            ),
        };

    let signed_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let event = wallet_core::notification::SignEvent::EvmTransaction {
        network: network.display_name().to_string(),
        from,
        to,
        value,
        signed_at,
    };
    state
        .send_telegram(&wallet_core::notification::format_sign_message(&event))
        .await;

    let signed_hex = format!("0x{}", hex::encode(&signed_bytes));
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "request_id": rid,
            "network": req.network,
            "transaction": signed_hex
        })),
    )
}

#[utoipa::path(
    post,
    path = "/api/wallet/sign/evm/typed-data",
    request_body = SignEvmTypedDataRequest,
    responses(
        (status = 200, description = "签名成功", body = SignEvmTypedDataResponse),
        (status = 400, description = "请求参数错误（含 missing_request_id / invalid_network / invalid_typed_data / chain_id_mismatch）", body = ErrorResponse),
        (status = 404, description = "该网络尚未创建钱包", body = ErrorResponse),
        (status = 415, description = "Content-Type 不是 application/json，响应不含 request_id", body = ErrorResponse),
        (status = 422, description = "JSON 结构正确但字段类型不匹配，响应不含 request_id", body = ErrorResponse),
        (status = 500, description = "内部签名错误", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "EIP-712 结构化数据签名",
    description = "对 EIP-712 typed data 进行签名，返回完整 ECDSA 签名及 r/s/v 分量。\n\n注意：415 和 422 错误发生在请求解析阶段，响应中不含 request_id，这是预期行为。",
)]
async fn sign_evm_typed_data(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SignEvmTypedDataRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let rid = match req.request_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing_request_id"})),
        ),
    };
    let rid = rid.as_str();

    let network = match crate::util::parse_network(&req.network) {
        Some(n) if n != wallet_core::Network::Solana => n,
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_network", "request_id": rid})),
        ),
    };
    let chain_id = network.chain_id().unwrap();

    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign/evm/typed-data")).await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})),
        );
    }

    let private_key = match state.wallet.get_private_key_bytes(&network).await {
        Ok(k) => k,
        Err(wallet_core::wallet::WalletError::Locked) => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})),
        ),
        Err(wallet_core::wallet::WalletError::NotFound(_)) => return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "wallet_not_found", "request_id": rid})),
        ),
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
        ),
    };

    let (signature, r, s, v) =
        match wallet_core::evm_wallet::sign_typed_data(&private_key, &req.typed_data, chain_id) {
            Ok(result) => result,
            Err(wallet_core::wallet::WalletError::ChainIdMismatch { expected, actual }) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "chain_id_mismatch",
                    "message": format!("expected {expected}, got {actual}"),
                    "request_id": rid
                })),
            ),
            Err(wallet_core::wallet::WalletError::InvalidTypedData(msg)) => return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_typed_data", "message": msg, "request_id": rid})),
            ),
            Err(e) => return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
            ),
        };

    let signed_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let typed_data_json_str = serde_json::to_string(&req.typed_data)
        .unwrap_or_else(|_| "<serialize error>".to_string());
    let event = wallet_core::notification::SignEvent::EvmTypedData {
        network: network.display_name().to_string(),
        typed_data_json: typed_data_json_str,
        signed_at,
    };
    state
        .send_telegram(&wallet_core::notification::format_sign_message(&event))
        .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "request_id": rid,
            "network": req.network,
            "signature": signature,
            "r": r,
            "s": s,
            "v": v
        })),
    )
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
    async fn get_address_with_invalid_network_returns_400_or_503() {
        let (server, _dir) = locked_server();
        let resp = server
            .get("/api/wallet/address")
            .add_query_param("network", "invalid_network")
            .await;
        assert!(resp.status_code() == 503 || resp.status_code() == 400);
    }

    #[tokio::test]
    async fn old_sign_endpoint_returns_404() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let server = TestServer::new(router(state)).unwrap();
        let resp = server
            .post("/api/wallet/sign")
            .json(&serde_json::json!({"network": "eth", "transaction": "abc"}))
            .await;
        assert_eq!(resp.status_code(), 404);
    }

    #[tokio::test]
    async fn openapi_does_not_contain_old_sign_path() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let server = TestServer::new(router(state)).unwrap();
        let resp = server.get("/api/docs/openapi.json").await;
        let body: serde_json::Value = resp.json();

        // Old path removed, three new paths present
        assert!(body["paths"].get("/api/wallet/sign").is_none());
        assert!(body["paths"]["/api/wallet/sign/solana"].is_object());
        assert!(body["paths"]["/api/wallet/sign/evm/transaction"].is_object());
        assert!(body["paths"]["/api/wallet/sign/evm/typed-data"].is_object());

        let schemas = &body["components"]["schemas"];

        // typed_data field declared as object type
        assert_eq!(
            schemas["SignEvmTypedDataRequest"]["properties"]["typed_data"]["type"],
            "object",
            "typed_data must be declared as object in OpenAPI schema"
        );

        // Old schemas removed
        assert!(schemas["SignRequest"].is_null(), "SignRequest schema should not exist");
        assert!(schemas["SignMetadata"].is_null(), "SignMetadata schema should not exist");
        assert!(schemas["SignResponse"].is_null(), "SignResponse schema should not exist");

        // request_id is required in all three request schemas
        let solana_req = &schemas["SignSolanaRequest"];
        let evm_tx_req = &schemas["SignEvmTransactionRequest"];
        let evm_td_req = &schemas["SignEvmTypedDataRequest"];
        assert!(
            solana_req["required"].as_array().unwrap().iter().any(|v| v == "request_id"),
            "SignSolanaRequest.request_id must be required"
        );
        assert!(
            evm_tx_req["required"].as_array().unwrap().iter().any(|v| v == "request_id"),
            "SignEvmTransactionRequest.request_id must be required"
        );
        assert!(
            evm_td_req["required"].as_array().unwrap().iter().any(|v| v == "request_id"),
            "SignEvmTypedDataRequest.request_id must be required"
        );
    }

    mod solana_sign_tests {
        use super::*;
        use axum_test::TestServer;
        use solana_sdk::{
            hash::Hash,
            message::{Message, VersionedMessage},
            pubkey::Pubkey,
            signature::Keypair as SolanaKeypair,
            signer::Signer,
            transaction::VersionedTransaction,
        };

        fn make_unsigned_solana_tx_base64(kp: &SolanaKeypair) -> String {
            let ix = solana_sdk::system_instruction::transfer(
                &kp.pubkey(),
                &Pubkey::new_unique(),
                1_000_000,
            );
            let msg = Message::new(&[ix], Some(&kp.pubkey()));
            let mut tx = solana_sdk::transaction::Transaction::new_unsigned(msg);
            tx.message.recent_blockhash = Hash::new_unique();
            let bytes = bincode::serialize(&tx).unwrap();
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        }

        fn make_unsigned_versioned_tx_base64(kp: &SolanaKeypair) -> String {
            let ix = solana_sdk::system_instruction::transfer(
                &kp.pubkey(),
                &Pubkey::new_unique(),
                1_000_000,
            );
            let blockhash = Hash::new_unique();
            let msg_v0 = solana_sdk::message::v0::Message::try_compile(
                &kp.pubkey(),
                &[ix],
                &[],
                blockhash,
            )
            .unwrap();
            let vtx = VersionedTransaction {
                signatures: vec![solana_sdk::signature::Signature::default()],
                message: VersionedMessage::V0(msg_v0),
            };
            let bytes = bincode::serialize(&vtx).unwrap();
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        }

        async fn unlocked_server_with_solana(
        ) -> (TestServer, tempfile::TempDir, SolanaKeypair) {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let password = "test-pass";
            let kp = SolanaKeypair::new();
            let keys = wallet_core::WalletKeys {
                private_key_bytes: kp.to_bytes().to_vec(),
                address: kp.pubkey().to_string(),
            };
            state
                .wallet
                .save_wallet(&wallet_core::Network::Solana, &keys, password)
                .unwrap();
            state.wallet.unlock(password).await.unwrap();
            let app = router(state);
            (TestServer::new(app).unwrap(), dir, kp)
        }

        #[tokio::test]
        async fn sign_solana_locked_returns_503() {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let server = TestServer::new(router(state)).unwrap();
            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "encoding": "base64",
                    "transaction": "abc"
                }))
                .await;
            assert_eq!(resp.status_code(), 503);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "wallet_locked");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_solana_missing_request_id_returns_400() {
            let (server, _dir, _kp) = unlocked_server_with_solana().await;
            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "encoding": "base64",
                    "transaction": "abc"
                }))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "missing_request_id");
            assert!(body.get("request_id").is_none());
        }

        #[tokio::test]
        async fn sign_solana_invalid_encoding_returns_400() {
            let (server, _dir, _kp) = unlocked_server_with_solana().await;
            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "encoding": "hex",
                    "transaction": "abc"
                }))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "invalid_encoding");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_solana_invalid_transaction_returns_400() {
            let (server, _dir, _kp) = unlocked_server_with_solana().await;
            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "encoding": "base64",
                    "transaction": "bm90YXZhbGlkdHg="
                }))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "invalid_transaction");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_solana_legacy_success_returns_base58_transaction() {
            let (server, _dir, kp) = unlocked_server_with_solana().await;
            let tx_b64 = make_unsigned_solana_tx_base64(&kp);
            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "encoding": "base64",
                    "transaction": tx_b64
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["request_id"], "req-1");

            let tx_str = body["transaction"].as_str().unwrap();
            let decoded = bs58::decode(tx_str).into_vec().unwrap();
            let signed_tx: solana_sdk::transaction::Transaction =
                bincode::deserialize(&decoded).unwrap();
            assert!(signed_tx.verify().is_ok());
        }

        #[tokio::test]
        async fn sign_solana_versioned_success_returns_base58_transaction() {
            let (server, _dir, kp) = unlocked_server_with_solana().await;
            let tx_b64 = make_unsigned_versioned_tx_base64(&kp);
            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "encoding": "base64",
                    "transaction": tx_b64
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["request_id"], "req-1");

            // Response must be a valid base58 string decodable as VersionedTransaction
            let tx_str = body["transaction"].as_str().unwrap();
            let decoded = bs58::decode(tx_str).into_vec().unwrap();
            let signed_vtx: VersionedTransaction = bincode::deserialize(&decoded).unwrap();

            // Signer slot (index 0 = fee payer) must be filled
            let default_sig = solana_sdk::signature::Signature::default();
            assert_ne!(
                signed_vtx.signatures[0], default_sig,
                "fee payer signature slot should be filled after signing"
            );
        }

        #[tokio::test]
        async fn sign_solana_all_errors_contain_request_id() {
            let (server, _dir, _kp) = unlocked_server_with_solana().await;
            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "request_id": "my-trace-id",
                    "encoding": "invalid",
                    "transaction": "abc"
                }))
                .await;
            let body: serde_json::Value = resp.json();
            assert_eq!(body["request_id"], "my-trace-id");
        }

        #[tokio::test]
        async fn sign_solana_wallet_not_found_returns_404() {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let password = "test-pass";
            let keys = wallet_core::evm_wallet::generate_keypair();
            state
                .wallet
                .save_wallet(&wallet_core::Network::Eth, &keys, password)
                .unwrap();
            state.wallet.unlock(password).await.unwrap();
            let server = TestServer::new(router(state)).unwrap();

            let resp = server
                .post("/api/wallet/sign/solana")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "encoding": "base64",
                    "transaction": "AAAA"
                }))
                .await;
            assert_eq!(resp.status_code(), 404);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "wallet_not_found");
            assert_eq!(body["request_id"], "req-1");
        }
    }

    mod evm_tx_sign_tests {
        use super::*;
        use alloy::consensus::{SignableTransaction, TxEnvelope, TxEip1559};
        use alloy::eips::eip2718::Encodable2718;
        use alloy::primitives::{Address, Bytes, Signature as AlloySignature, TxKind, U256};
        use axum_test::TestServer;

        fn make_unsigned_type2_hex(chain_id: u64) -> String {
            let tx = TxEip1559 {
                chain_id,
                nonce: 0,
                max_priority_fee_per_gas: 1_000_000_000,
                max_fee_per_gas: 20_000_000_000,
                gas_limit: 21000,
                to: TxKind::Call(Address::repeat_byte(0xde)),
                value: U256::from(1_000_000_000_000_000_000u64),
                input: Bytes::default(),
                access_list: Default::default(),
            };
            let zero_sig = AlloySignature::new(U256::ZERO, U256::ZERO, false);
            let signed = tx.into_signed(zero_sig);
            let mut out = Vec::new();
            TxEnvelope::Eip1559(signed).encode_2718(&mut out);
            format!("0x{}", hex::encode(&out))
        }

        async fn unlocked_eth_server() -> (TestServer, tempfile::TempDir) {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let password = "test-pass";
            let keys = wallet_core::evm_wallet::generate_keypair();
            state
                .wallet
                .save_wallet(&wallet_core::Network::Eth, &keys, password)
                .unwrap();
            state.wallet.unlock(password).await.unwrap();
            (TestServer::new(router(state)).unwrap(), dir)
        }

        #[tokio::test]
        async fn sign_evm_tx_locked_returns_503_with_request_id() {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let server = TestServer::new(router(state)).unwrap();
            let resp = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "transaction": "0xaabbcc"
                }))
                .await;
            assert_eq!(resp.status_code(), 503);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["request_id"], "req-1");
            assert_eq!(body["error"], "wallet_locked");
        }

        #[tokio::test]
        async fn sign_evm_tx_invalid_network_returns_400() {
            let (server, _dir) = unlocked_eth_server().await;
            let resp = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "invalid",
                    "transaction": "0x00"
                }))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "invalid_network");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_evm_tx_type2_success_sender_recoverable() {
            let (server, _dir) = unlocked_eth_server().await;
            let tx_hex = make_unsigned_type2_hex(1);
            let resp = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "transaction": tx_hex
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["request_id"], "req-1");
            assert_eq!(body["network"], "eth");
            let signed_hex = body["transaction"].as_str().unwrap();
            assert!(signed_hex.starts_with("0x"));
        }

        #[tokio::test]
        async fn sign_evm_tx_already_signed_returns_400() {
            let (server, _dir) = unlocked_eth_server().await;
            let tx_hex = make_unsigned_type2_hex(1);
            let resp1 = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({"request_id": "req-1", "network": "eth", "transaction": tx_hex}))
                .await;
            let signed_hex = resp1.json::<serde_json::Value>()["transaction"]
                .as_str()
                .unwrap()
                .to_string();

            let resp2 = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({"request_id": "req-2", "network": "eth", "transaction": signed_hex}))
                .await;
            assert_eq!(resp2.status_code(), 400);
            let body: serde_json::Value = resp2.json();
            assert_eq!(body["error"], "already_signed");
            assert_eq!(body["request_id"], "req-2");
        }

        #[tokio::test]
        async fn sign_evm_tx_chain_id_mismatch_returns_400() {
            let (server, _dir) = unlocked_eth_server().await;
            let tx_hex = make_unsigned_type2_hex(56);
            let resp = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({"request_id": "req-1", "network": "eth", "transaction": tx_hex}))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "chain_id_mismatch");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_evm_tx_invalid_hex_returns_400() {
            let (server, _dir) = unlocked_eth_server().await;
            let resp = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({"request_id": "req-1", "network": "eth", "transaction": "not-hex"}))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "invalid_transaction");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_evm_tx_wallet_not_found_returns_404() {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let password = "test-pass";
            let keys = wallet_core::evm_wallet::generate_keypair();
            state
                .wallet
                .save_wallet(&wallet_core::Network::Eth, &keys, password)
                .unwrap();
            state.wallet.unlock(password).await.unwrap();
            let server = TestServer::new(router(state)).unwrap();

            let tx_hex = make_unsigned_type2_hex(137);
            let resp = server
                .post("/api/wallet/sign/evm/transaction")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "polygon",
                    "transaction": tx_hex
                }))
                .await;
            assert_eq!(resp.status_code(), 404);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "wallet_not_found");
            assert_eq!(body["request_id"], "req-1");
        }
    }

    mod eip712_sign_tests {
        use super::*;
        use axum_test::TestServer;

        async fn unlocked_eth_server() -> (TestServer, tempfile::TempDir) {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let password = "test-pass";
            let keys = wallet_core::evm_wallet::generate_keypair();
            state
                .wallet
                .save_wallet(&wallet_core::Network::Eth, &keys, password)
                .unwrap();
            state.wallet.unlock(password).await.unwrap();
            (TestServer::new(router(state)).unwrap(), dir)
        }

        fn valid_typed_data(chain_id: u64) -> serde_json::Value {
            serde_json::json!({
                "domain": {
                    "name": "TestApp",
                    "version": "1",
                    "chainId": chain_id,
                    "verifyingContract": "0x0000000000000000000000000000000000000001"
                },
                "types": {
                    "Transfer": [
                        {"name": "to",    "type": "address"},
                        {"name": "value", "type": "uint256"}
                    ]
                },
                "primaryType": "Transfer",
                "message": {
                    "to": "0x0000000000000000000000000000000000000002",
                    "value": "1000000"
                }
            })
        }

        #[tokio::test]
        async fn sign_eip712_locked_returns_503() {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let server = TestServer::new(router(state)).unwrap();
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": valid_typed_data(1)
                }))
                .await;
            assert_eq!(resp.status_code(), 503);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "wallet_locked");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_eip712_invalid_network_returns_400() {
            let (server, _dir) = unlocked_eth_server().await;
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "unknown",
                    "typed_data": valid_typed_data(1)
                }))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "invalid_network");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_eip712_success_returns_r_s_v_signature() {
            let (server, _dir) = unlocked_eth_server().await;
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": valid_typed_data(1)
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["request_id"], "req-1");
            assert_eq!(body["network"], "eth");

            let sig = body["signature"].as_str().unwrap();
            let r = body["r"].as_str().unwrap();
            let s = body["s"].as_str().unwrap();
            let v = body["v"].as_str().unwrap();

            assert_eq!(sig.len(), 132);
            assert_eq!(r.len(), 66);
            assert_eq!(s.len(), 66);
            assert!(v == "0x1b" || v == "0x1c");

            let combined = format!("0x{}{}{}", &r[2..], &s[2..], &v[2..]);
            assert_eq!(combined, sig);
        }

        #[tokio::test]
        async fn sign_eip712_chain_id_mismatch_returns_400() {
            let (server, _dir) = unlocked_eth_server().await;
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": valid_typed_data(56)
                }))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "chain_id_mismatch");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_eip712_missing_chain_id_injects_and_succeeds() {
            let (server, _dir) = unlocked_eth_server().await;
            let mut td = valid_typed_data(1);
            td["domain"].as_object_mut().unwrap().remove("chainId");
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": td
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
        }

        #[tokio::test]
        async fn sign_eip712_message_extra_field_returns_400() {
            let (server, _dir) = unlocked_eth_server().await;
            let mut td = valid_typed_data(1);
            td["message"]["extra"] = serde_json::json!("unexpected");
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": td
                }))
                .await;
            assert_eq!(resp.status_code(), 400);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "invalid_typed_data");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_eip712_eip712domain_in_types_is_ignored() {
            let (server, _dir) = unlocked_eth_server().await;
            let mut td = valid_typed_data(1);
            td["types"]["EIP712Domain"] =
                serde_json::json!([{"name": "name", "type": "string"}]);
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": td
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
        }

        #[tokio::test]
        async fn sign_eip712_wallet_not_found_returns_404() {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(dir.path().to_path_buf());
            let password = "test-pass";
            let keys = wallet_core::evm_wallet::generate_keypair();
            state
                .wallet
                .save_wallet(&wallet_core::Network::Eth, &keys, password)
                .unwrap();
            state.wallet.unlock(password).await.unwrap();
            let server = TestServer::new(router(state)).unwrap();

            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "bnb",
                    "typed_data": valid_typed_data(56)
                }))
                .await;
            assert_eq!(resp.status_code(), 404);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["error"], "wallet_not_found");
            assert_eq!(body["request_id"], "req-1");
        }

        #[tokio::test]
        async fn sign_eip712_nested_struct_returns_200() {
            let (server, _dir) = unlocked_eth_server().await;
            let td = serde_json::json!({
                "domain": {"name": "TestApp", "version": "1", "chainId": 1},
                "types": {
                    "Order": [
                        {"name": "item",     "type": "OrderItem"},
                        {"name": "deadline", "type": "uint256"}
                    ],
                    "OrderItem": [
                        {"name": "recipient", "type": "address"},
                        {"name": "amount",    "type": "uint256"}
                    ]
                },
                "primaryType": "Order",
                "message": {
                    "item": {
                        "recipient": "0x0000000000000000000000000000000000000001",
                        "amount": "1000"
                    },
                    "deadline": "9999"
                }
            });
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": td
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["signature"].as_str().unwrap().len(), 132);
        }

        #[tokio::test]
        async fn sign_eip712_array_type_returns_200() {
            let (server, _dir) = unlocked_eth_server().await;
            let td = serde_json::json!({
                "domain": {"name": "TestApp", "version": "1", "chainId": 1},
                "types": {
                    "Batch": [
                        {"name": "recipients", "type": "address[]"},
                        {"name": "amounts",    "type": "uint256[]"}
                    ]
                },
                "primaryType": "Batch",
                "message": {
                    "recipients": [
                        "0x0000000000000000000000000000000000000001",
                        "0x0000000000000000000000000000000000000002"
                    ],
                    "amounts": ["100", "200"]
                }
            });
            let resp = server
                .post("/api/wallet/sign/evm/typed-data")
                .json(&serde_json::json!({
                    "request_id": "req-1",
                    "network": "eth",
                    "typed_data": td
                }))
                .await;
            assert_eq!(resp.status_code(), 200);
            let body: serde_json::Value = resp.json();
            assert_eq!(body["signature"].as_str().unwrap().len(), 132);
        }
    }
}
