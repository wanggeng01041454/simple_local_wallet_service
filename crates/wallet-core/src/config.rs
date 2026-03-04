use crate::crypto;
use crate::wallet::WalletError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub solana: String,
    pub eth: String,
    pub bnb: String,
    pub arb: String,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            solana: "https://api.mainnet-beta.solana.com".to_string(),
            eth: "https://eth.llamarpc.com".to_string(),
            bnb: "https://bsc-dataseed.binance.org".to_string(),
            arb: "https://arb1.arbitrum.io/rpc".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub rpc: RpcConfig,
    #[serde(skip)]
    data_dir: PathBuf,
}

impl AppConfig {
    pub fn load_or_default(data_dir: PathBuf) -> Self {
        let path = data_dir.join("config.json");
        if path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&raw) {
                    config.data_dir = data_dir;
                    return config;
                }
            }
        }
        Self {
            rpc: RpcConfig::default(),
            data_dir,
        }
    }

    pub fn save(&self) -> Result<(), WalletError> {
        std::fs::create_dir_all(&self.data_dir)?;
        let path = self.data_dir.join("config.json");
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn save_telegram(&self, tg: &TelegramConfig, password: &str) -> Result<(), WalletError> {
        let plaintext = serde_json::to_vec(tg)?;
        let enc = crypto::encrypt(&plaintext, password)?;
        std::fs::create_dir_all(&self.data_dir)?;
        let path = self.data_dir.join("secret.enc");
        std::fs::write(path, serde_json::to_string_pretty(&enc)?)?;
        Ok(())
    }

    pub fn load_telegram(&self, password: &str) -> Result<Option<TelegramConfig>, WalletError> {
        let path = self.data_dir.join("secret.enc");
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path)?;
        let enc = serde_json::from_str(&raw)?;
        let plaintext = crypto::decrypt(&enc, password)?;
        let tg = serde_json::from_slice(&plaintext)?;
        Ok(Some(tg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(dir: &tempfile::TempDir) -> AppConfig {
        AppConfig::load_or_default(dir.path().to_path_buf())
    }

    #[test]
    fn load_or_default_returns_defaults_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = temp_config(&dir);
        assert_eq!(config.rpc.solana, "https://api.mainnet-beta.solana.com");
        assert_eq!(config.rpc.eth, "https://eth.llamarpc.com");
    }

    #[test]
    fn save_and_reload_roundtrips_rpc_urls() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = temp_config(&dir);
        config.rpc.eth = "https://my-custom-rpc.example.com".to_string();
        config.save().unwrap();

        let reloaded = AppConfig::load_or_default(dir.path().to_path_buf());
        assert_eq!(reloaded.rpc.eth, "https://my-custom-rpc.example.com");
    }

    #[test]
    fn save_and_load_telegram_config_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let config = temp_config(&dir);

        let tg = TelegramConfig {
            bot_token: "my_bot_token".to_string(),
            chat_id: "123456".to_string(),
        };
        config.save_telegram(&tg, "my-password").unwrap();

        let loaded = config.load_telegram("my-password").unwrap().unwrap();
        assert_eq!(loaded.bot_token, "my_bot_token");
        assert_eq!(loaded.chat_id, "123456");
    }

    #[test]
    fn load_telegram_with_wrong_password_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let config = temp_config(&dir);
        let tg = TelegramConfig {
            bot_token: "token".to_string(),
            chat_id: "id".to_string(),
        };
        config.save_telegram(&tg, "correct").unwrap();

        let result = config.load_telegram("wrong");
        assert!(result.is_err());
    }
}
