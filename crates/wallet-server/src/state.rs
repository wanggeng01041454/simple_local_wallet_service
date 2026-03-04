use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use wallet_core::{config::AppConfig, notification::TelegramClient, WalletManager};

#[derive(Clone)]
pub struct AppState {
    pub wallet: Arc<WalletManager>,
    pub config: Arc<RwLock<AppConfig>>,
    pub telegram: Arc<RwLock<Option<TelegramClient>>>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let config = AppConfig::load_or_default(data_dir.clone());
        Self {
            wallet: Arc::new(WalletManager::new(data_dir)),
            config: Arc::new(RwLock::new(config)),
            telegram: Arc::new(RwLock::new(None)),
        }
    }

    /// Load Telegram config (called after wallet unlock, when password is available).
    pub async fn load_telegram(&self, password: &str) {
        let config = self.config.read().await;
        match config.load_telegram(password) {
            Ok(Some(tg)) => {
                *self.telegram.write().await =
                    Some(TelegramClient::new(tg.bot_token, tg.chat_id));
            }
            Ok(None) => {}
            Err(e) => tracing::error!("failed to load telegram config: {e}"),
        }
    }

    pub async fn send_telegram(&self, message: &str) {
        if let Some(client) = &*self.telegram.read().await {
            client.send(message).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_new_creates_locked_wallet() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        assert_eq!(Arc::strong_count(&state.wallet), 1);
    }
}
