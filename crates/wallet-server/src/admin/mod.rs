use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use wallet_core::{
    config::TelegramConfig, evm_wallet, network::Network, solana_wallet, wallet::WalletKeys,
};

use crate::state::AppState;
use crate::util::parse_network;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/admin/health", get(health))
        .route("/api/admin/status", get(get_status))
        .route("/api/admin/unlock", post(unlock))
        .route("/api/admin/lock", post(lock))
        .route("/api/admin/setup/wallet", post(setup_wallet))
        .route("/api/admin/settings/telegram", post(save_telegram_settings))
        .route("/api/admin/settings/rpc", post(save_rpc_settings))
        .route("/api/admin/settings", get(get_settings))
        .fallback(serve_frontend)
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn get_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let unlocked = state.wallet.is_unlocked().await;
    let setup_required = !state.wallet.has_any_wallet();
    Json(serde_json::json!({
        "unlocked": unlocked,
        "setup_required": setup_required,
    }))
}

#[derive(Deserialize)]
struct UnlockRequest {
    password: String,
}

async fn unlock(
    State(state): State<AppState>,
    Json(req): Json<UnlockRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.wallet.unlock(&req.password).await {
        Ok(_) => {
            state.load_telegram(&req.password).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({"status": "unlocked"})),
            )
        }
        Err(wallet_core::wallet::WalletError::NotFound(_)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "no wallets found"})),
        ),
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid password"})),
        ),
    }
}

async fn lock(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.wallet.lock().await;
    Json(serde_json::json!({"status": "locked"}))
}

#[derive(Deserialize)]
struct SetupWalletRequest {
    password: String,
    network: String,
    action: String,
    private_key: Option<String>,
}

async fn setup_wallet(
    State(state): State<AppState>,
    Json(req): Json<SetupWalletRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let network = match parse_network(&req.network) {
        Some(n) => n,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid network"})),
            )
        }
    };

    let keys: WalletKeys = match req.action.as_str() {
        "generate" => match network {
            Network::Solana => solana_wallet::generate_keypair(),
            Network::Eth | Network::Bnb | Network::Arb | Network::Polygon => {
                evm_wallet::generate_keypair()
            }
        },
        "import" => {
            let pk = match &req.private_key {
                Some(k) => k,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "private_key required for import"})),
                    )
                }
            };
            let result = match network {
                Network::Solana => solana_wallet::import_from_base58(pk),
                Network::Eth | Network::Bnb | Network::Arb | Network::Polygon => {
                    evm_wallet::import_from_hex(pk)
                }
            };
            match result {
                Ok(k) => k,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                }
            }
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "action must be generate or import"})),
            )
        }
    };

    let address = keys.address.clone();
    if let Err(e) = state.wallet.save_wallet(&network, &keys, &req.password) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "network": req.network,
            "address": address
        })),
    )
}

#[derive(Deserialize)]
struct TelegramSettingsRequest {
    bot_token: String,
    chat_id: String,
    password: String,
}

async fn save_telegram_settings(
    State(state): State<AppState>,
    Json(req): Json<TelegramSettingsRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.wallet.verify_password(&req.password).await {
        Ok(()) => {}
        Err(wallet_core::wallet::WalletError::NotFound(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "no wallets found"})),
            )
        }
        Err(wallet_core::wallet::WalletError::Crypto(_)) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid password"})),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }

    let tg = TelegramConfig {
        bot_token: req.bot_token.clone(),
        chat_id: req.chat_id.clone(),
    };
    let config = state.config.read().await;
    match config.save_telegram(&tg, &req.password) {
        Ok(_) => {
            drop(config);
            state.load_telegram(&req.password).await;
            (StatusCode::OK, Json(serde_json::json!({"status": "saved"})))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

#[derive(Deserialize)]
struct RpcSettingsRequest {
    solana: Option<String>,
    eth: Option<String>,
    bnb: Option<String>,
    arb: Option<String>,
    polygon: Option<String>,
}

async fn save_rpc_settings(
    State(state): State<AppState>,
    Json(req): Json<RpcSettingsRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut config = state.config.write().await;
    if let Some(url) = req.solana {
        config.rpc.solana = url;
    }
    if let Some(url) = req.eth {
        config.rpc.eth = url;
    }
    if let Some(url) = req.bnb {
        config.rpc.bnb = url;
    }
    if let Some(url) = req.arb {
        config.rpc.arb = url;
    }
    if let Some(url) = req.polygon {
        config.rpc.polygon = url;
    }
    match config.save() {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "saved"}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn get_settings(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(serde_json::json!({
        "rpc": {
            "solana": config.rpc.solana,
            "eth": config.rpc.eth,
            "bnb": config.rpc.bnb,
            "arb": config.rpc.arb,
            "polygon": config.rpc.polygon,
        }
    }))
}

async fn serve_frontend(uri: axum::http::Uri) -> axum::response::Response<axum::body::Body> {
    use axum::body::Body;
    use axum::response::Response;
    use http::{header, StatusCode};

    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    let dir = &crate::static_files::FRONTEND_DIR;

    match dir.get_file(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(file.contents()))
                .unwrap()
        }
        None => {
            // SPA fallback — serve index.html for all unknown routes
            let index = dir.get_file("index.html").unwrap();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html")
                .body(Body::from(index.contents()))
                .unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use tempfile::TempDir;

    fn admin_server() -> (TestServer, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let app = router(state);
        (TestServer::new(app).unwrap(), dir)
    }

    #[tokio::test]
    async fn get_status_returns_setup_required_when_no_wallets() {
        let (server, _dir) = admin_server();
        let resp = server.get("/api/admin/status").await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["setup_required"], true);
        assert_eq!(body["unlocked"], false);
    }

    #[tokio::test]
    async fn unlock_with_no_wallets_returns_400() {
        let (server, _dir) = admin_server();
        let resp = server
            .post("/api/admin/unlock")
            .json(&serde_json::json!({"password": "test123"}))
            .await;
        assert_eq!(resp.status_code(), 400);
    }

    #[tokio::test]
    async fn setup_creates_wallet_and_can_unlock() {
        let (server, _dir) = admin_server();

        let resp = server
            .post("/api/admin/setup/wallet")
            .json(&serde_json::json!({
                "password": "test-password-123",
                "network": "eth",
                "action": "generate"
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert!(body["address"].as_str().unwrap().starts_with("0x"));

        let resp = server
            .post("/api/admin/unlock")
            .json(&serde_json::json!({"password": "test-password-123"}))
            .await;
        assert_eq!(resp.status_code(), 200);

        let resp = server.get("/api/admin/status").await;
        let body: serde_json::Value = resp.json();
        assert_eq!(body["unlocked"], true);
    }

    #[tokio::test]
    async fn unlock_with_wrong_password_returns_401() {
        let (server, _dir) = admin_server();
        server
            .post("/api/admin/setup/wallet")
            .json(&serde_json::json!({
                "password": "correct",
                "network": "eth",
                "action": "generate"
            }))
            .await;

        let resp = server
            .post("/api/admin/unlock")
            .json(&serde_json::json!({"password": "wrong"}))
            .await;
        assert_eq!(resp.status_code(), 401);
    }

    #[tokio::test]
    async fn save_telegram_settings_with_wrong_password_returns_401() {
        let (server, _dir) = admin_server();
        server
            .post("/api/admin/setup/wallet")
            .json(&serde_json::json!({
                "password": "correct",
                "network": "eth",
                "action": "generate"
            }))
            .await;

        let resp = server
            .post("/api/admin/settings/telegram")
            .json(&serde_json::json!({
                "bot_token": "bot-token",
                "chat_id": "chat-id",
                "password": "wrong"
            }))
            .await;

        assert_eq!(resp.status_code(), 401);
    }
}
