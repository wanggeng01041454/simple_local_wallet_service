use crate::crypto::{self, EncryptedData};
use crate::network::Network;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum WalletError {
    #[error("wallet is locked — unlock via admin UI at http://localhost:9292")]
    Locked,
    #[error("wallet file not found for network: {0}")]
    NotFound(String),
    #[error("encryption error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletKeys {
    pub private_key_bytes: Vec<u8>,
    pub address: String,
}

enum WalletState {
    Locked,
    Unlocked(HashMap<Network, WalletKeys>),
}

pub struct WalletManager {
    state: RwLock<WalletState>,
    data_dir: PathBuf,
}

impl WalletManager {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            state: RwLock::new(WalletState::Locked),
            data_dir,
        }
    }

    pub fn wallets_dir(&self) -> PathBuf {
        self.data_dir.join("wallets")
    }

    pub async fn is_unlocked(&self) -> bool {
        matches!(*self.state.read().await, WalletState::Unlocked(_))
    }

    pub async fn lock(&self) {
        *self.state.write().await = WalletState::Locked;
    }

    pub async fn unlock(&self, password: &str) -> Result<(), WalletError> {
        let wallets_dir = self.wallets_dir();
        let password = password.to_string();

        // PBKDF2 with 600k iterations is CPU-intensive — run off the async executor
        let keys = tokio::task::spawn_blocking(move || {
            let mut keys = HashMap::new();
            for network in Network::all() {
                let path = wallets_dir.join(network.wallet_filename());
                if !path.exists() {
                    continue;
                }
                let raw = std::fs::read_to_string(&path)?;
                let enc: EncryptedData = serde_json::from_str(&raw)?;
                let decrypted = crypto::decrypt(&enc, &password)?;
                let wallet_keys: WalletKeys = serde_json::from_slice(&decrypted)?;
                keys.insert(network.clone(), wallet_keys);
            }
            Ok::<_, WalletError>(keys)
        })
        .await
        .expect("unlock task panicked")?;

        if keys.is_empty() {
            return Err(WalletError::NotFound("no wallet files found".to_string()));
        }

        *self.state.write().await = WalletState::Unlocked(keys);
        Ok(())
    }

    pub async fn verify_password(&self, password: &str) -> Result<(), WalletError> {
        let wallets_dir = self.wallets_dir();
        let password = password.to_string();

        tokio::task::spawn_blocking(move || {
            for network in Network::all() {
                let path = wallets_dir.join(network.wallet_filename());
                if !path.exists() {
                    continue;
                }
                let raw = std::fs::read_to_string(&path)?;
                let enc: EncryptedData = serde_json::from_str(&raw)?;
                crypto::decrypt(&enc, &password)?;
                return Ok(());
            }
            Err(WalletError::NotFound("no wallet files found".to_string()))
        })
        .await
        .expect("verify password task panicked")
    }

    pub async fn get_address(&self, network: &Network) -> Result<String, WalletError> {
        match &*self.state.read().await {
            WalletState::Locked => Err(WalletError::Locked),
            WalletState::Unlocked(keys) => keys
                .get(network)
                .map(|k| k.address.clone())
                .ok_or_else(|| WalletError::NotFound(network.to_string())),
        }
    }

    pub async fn get_private_key_bytes(&self, network: &Network) -> Result<Vec<u8>, WalletError> {
        match &*self.state.read().await {
            WalletState::Locked => Err(WalletError::Locked),
            WalletState::Unlocked(keys) => keys
                .get(network)
                .map(|k| k.private_key_bytes.clone())
                .ok_or_else(|| WalletError::NotFound(network.to_string())),
        }
    }

    pub fn save_wallet(
        &self,
        network: &Network,
        keys: &WalletKeys,
        password: &str,
    ) -> Result<(), WalletError> {
        let wallets_dir = self.wallets_dir();
        std::fs::create_dir_all(&wallets_dir)?;
        let plaintext = serde_json::to_vec(keys)?;
        let enc = crypto::encrypt(&plaintext, password)?;
        let path = wallets_dir.join(network.wallet_filename());
        std::fs::write(path, serde_json::to_string_pretty(&enc)?)?;
        Ok(())
    }

    pub fn has_any_wallet(&self) -> bool {
        Network::all()
            .iter()
            .any(|n| self.wallets_dir().join(n.wallet_filename()).exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_wallet_manager() -> (WalletManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let manager = WalletManager::new(dir.path().to_path_buf());
        (manager, dir)
    }

    #[tokio::test]
    async fn new_wallet_manager_starts_locked() {
        let (manager, _dir) = temp_wallet_manager();
        assert!(!manager.is_unlocked().await);
    }

    #[tokio::test]
    async fn unlock_with_no_wallet_files_returns_error() {
        let (manager, _dir) = temp_wallet_manager();
        let result = manager.unlock("any-password").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn lock_sets_state_to_locked() {
        let (manager, _dir) = temp_wallet_manager();
        manager.lock().await;
        assert!(!manager.is_unlocked().await);
    }

    #[tokio::test]
    async fn get_address_when_locked_returns_error() {
        let (manager, _dir) = temp_wallet_manager();
        let result = manager.get_address(&Network::Eth).await;
        assert!(matches!(result, Err(WalletError::Locked)));
    }
}
