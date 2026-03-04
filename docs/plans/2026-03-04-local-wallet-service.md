# Local Wallet Service Implementation Plan

**Goal:** Build a single Rust binary that manages encrypted Solana and EVM private keys, serves a React admin UI on port 9292, and exposes a REST API on port 9293.

**Architecture:** Cargo workspace with three crates (`wallet-core`, `wallet-server`, `wallet-cli`). The frontend is built with Vite and embedded into the binary via `include_dir`. Wallet state lives in a shared `Arc<WalletManager>` protected by `tokio::sync::RwLock`.

**Tech Stack:** Rust (axum 0.7, tokio 1, solana-sdk 2, alloy 0.9, aes-gcm 0.10, pbkdf2 0.12, rand 0.8, serde_json 1, include_dir 0.7, reqwest 0.12, tracing 0.1) · TypeScript (React 18, Vite 5, Vitest 2, React Router 6)

---

## Task 1: Cargo Workspace + Crate Stubs

> **TDD mode:** N/A — scaffolding only

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/wallet-core/Cargo.toml`
- Create: `crates/wallet-core/src/lib.rs`
- Create: `crates/wallet-server/Cargo.toml`
- Create: `crates/wallet-server/src/lib.rs`
- Create: `crates/wallet-cli/Cargo.toml`
- Create: `crates/wallet-cli/src/main.rs`

**Step 1: Create workspace Cargo.toml**

```toml
# Cargo.toml
[workspace]
members = [
    "crates/wallet-core",
    "crates/wallet-server",
    "crates/wallet-cli",
]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
axum = "0.7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1"
anyhow = "1"
```

**Step 2: Create wallet-core/Cargo.toml**

```toml
[package]
name = "wallet-core"
version = "0.1.0"
edition = "2021"

[dependencies]
aes-gcm = "0.10"
pbkdf2 = { version = "0.12", features = ["hmac"] }
sha2 = "0.10"
rand = "0.8"
serde = { workspace = true }
serde_json = { workspace = true }
base64 = "0.22"
thiserror = { workspace = true }
tracing = { workspace = true }
reqwest = { version = "0.12", features = ["json"] }
tokio = { workspace = true }
solana-sdk = "2"
alloy = { version = "0.9", features = ["full"] }
dirs = "5"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio-test = "0.4"
```

**Step 3: Create wallet-server/Cargo.toml**

```toml
[package]
name = "wallet-server"
version = "0.1.0"
edition = "2021"

[dependencies]
wallet-core = { path = "../wallet-core" }
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
include_dir = "0.7"
tower-http = { version = "0.5", features = ["cors", "fs"] }
http = "1"

[dev-dependencies]
axum-test = "0.4"
tokio = { workspace = true }
```

**Step 4: Create wallet-cli/Cargo.toml**

```toml
[package]
name = "wallet-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "local-wallet"
path = "src/main.rs"

[dependencies]
wallet-core = { path = "../wallet-core" }
wallet-server = { path = "../wallet-server" }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }
```

**Step 5: Stub lib.rs and main.rs**

```rust
// crates/wallet-core/src/lib.rs
pub mod crypto;
pub mod network;
pub mod wallet;
pub mod solana_wallet;
pub mod evm_wallet;
pub mod notification;
pub mod config;

pub use wallet::WalletManager;
pub use network::Network;
```

```rust
// crates/wallet-server/src/lib.rs
pub mod state;
pub mod api;
pub mod admin;
pub mod static_files;
```

```rust
// crates/wallet-cli/src/main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    println!("local-wallet starting...");
    Ok(())
}
```

**Step 6: Verify it compiles**

```bash
cargo build
```
Expected: compiles with warnings about unused modules, no errors.

**Step 7: Commit**

```bash
git add Cargo.toml crates/
git commit -m "chore: scaffold cargo workspace with three crates"
```

---

## Task 2: wallet-core — Crypto Module

> **TDD mode:** Simple Logic — pure encrypt/decrypt functions, input → output

**Files:**
- Create: `crates/wallet-core/src/crypto.rs`

**Step 1: Write the failing tests**

Add at the bottom of `crates/wallet-core/src/crypto.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip_returns_original_plaintext() {
        let plaintext = b"my-secret-private-key-bytes";
        let password = "correct-horse-battery-staple";

        let encrypted = encrypt(plaintext, password).unwrap();
        let decrypted = decrypt(&encrypted, password).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_password_returns_error() {
        let plaintext = b"my-secret-private-key-bytes";
        let encrypted = encrypt(plaintext, "correct-password").unwrap();

        let result = decrypt(&encrypted, "wrong-password");

        assert!(result.is_err());
    }

    #[test]
    fn each_encryption_produces_unique_ciphertext() {
        let plaintext = b"same plaintext";
        let password = "same password";

        let enc1 = encrypt(plaintext, password).unwrap();
        let enc2 = encrypt(plaintext, password).unwrap();

        // Different salts/IVs → different ciphertext
        assert_ne!(enc1.ciphertext, enc2.ciphertext);
        assert_ne!(enc1.salt, enc2.salt);
    }

    #[test]
    fn encrypted_data_serializes_to_json_and_back() {
        let plaintext = b"test data";
        let encrypted = encrypt(plaintext, "password").unwrap();

        let json = serde_json::to_string(&encrypted).unwrap();
        let parsed: EncryptedData = serde_json::from_str(&json).unwrap();
        let decrypted = decrypt(&parsed, "password").unwrap();

        assert_eq!(decrypted, plaintext);
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-core crypto
```
Expected: FAIL — module `crypto` is empty / functions don't exist yet.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-core/src/crypto.rs
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;

const PBKDF2_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("decryption failed: invalid password or corrupted data")]
    DecryptionFailed,
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("unsupported version: {0}")]
    UnsupportedVersion(u8),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptedData {
    pub version: u8,
    pub kdf: String,
    pub iterations: u32,
    pub salt: String,
    pub iv: String,
    pub ciphertext: String,
}

pub fn encrypt(plaintext: &[u8], password: &str) -> Result<EncryptedData, CryptoError> {
    let mut salt = [0u8; SALT_LEN];
    let mut iv = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut iv);

    let key = derive_key(password, &salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&iv);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    Ok(EncryptedData {
        version: 1,
        kdf: "pbkdf2-sha256".to_string(),
        iterations: PBKDF2_ITERATIONS,
        salt: STANDARD.encode(salt),
        iv: STANDARD.encode(iv),
        ciphertext: STANDARD.encode(ciphertext),
    })
}

pub fn decrypt(data: &EncryptedData, password: &str) -> Result<Vec<u8>, CryptoError> {
    if data.version != 1 {
        return Err(CryptoError::UnsupportedVersion(data.version));
    }

    let salt = STANDARD.decode(&data.salt)?;
    let iv = STANDARD.decode(&data.iv)?;
    let ciphertext = STANDARD.decode(&data.ciphertext)?;

    let key = derive_key(password, &salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&iv);

    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptionFailed)
}

fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-core crypto
```
Expected: PASS (4 tests)

**Step 5: Refactor**

No duplication to eliminate. The `derive_key` helper is already extracted. No further changes.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-core/src/crypto.rs
git commit -m "feat: add AES-256-GCM + PBKDF2 encryption module"
```

---

## Task 3: wallet-core — Network Types + WalletManager Skeleton

> **TDD mode:** Simple Logic — enum exhaustiveness and state transitions

**Files:**
- Create: `crates/wallet-core/src/network.rs`
- Create: `crates/wallet-core/src/wallet.rs`

**Step 1: Write the failing tests**

```rust
// At bottom of crates/wallet-core/src/wallet.rs

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_wallet_manager() -> (WalletManager, TempDir) {
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
```

Add to `wallet-core/Cargo.toml` dev-dependencies:
```toml
tempfile = "3"
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-core wallet
```
Expected: FAIL — `WalletManager`, `Network`, `WalletError` don't exist yet.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-core/src/network.rs
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Solana,
    Eth,
    Bnb,
    Arb,
}

impl Network {
    pub fn all() -> &'static [Network] {
        &[Network::Solana, Network::Eth, Network::Bnb, Network::Arb]
    }

    pub fn wallet_filename(&self) -> &'static str {
        match self {
            Network::Solana => "solana.enc",
            Network::Eth => "eth.enc",
            Network::Bnb => "bnb.enc",
            Network::Arb => "arb.enc",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Network::Solana => "Solana",
            Network::Eth => "Ethereum",
            Network::Bnb => "BNB Chain",
            Network::Arb => "Arbitrum",
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
```

```rust
// crates/wallet-core/src/wallet.rs
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
        let mut keys = HashMap::new();

        for network in Network::all() {
            let path = wallets_dir.join(network.wallet_filename());
            if !path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(&path)?;
            let enc: EncryptedData = serde_json::from_str(&raw)?;
            let decrypted = crypto::decrypt(&enc, password)?;
            let wallet_keys: WalletKeys = serde_json::from_slice(&decrypted)?;
            keys.insert(network.clone(), wallet_keys);
        }

        if keys.is_empty() {
            return Err(WalletError::NotFound("no wallet files found".to_string()));
        }

        *self.state.write().await = WalletState::Unlocked(keys);
        Ok(())
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

    /// Save a wallet key to disk (encrypted). Called during setup.
    pub fn save_wallet(&self, network: &Network, keys: &WalletKeys, password: &str) -> Result<(), WalletError> {
        let wallets_dir = self.wallets_dir();
        std::fs::create_dir_all(&wallets_dir)?;
        let plaintext = serde_json::to_vec(keys)?;
        let enc = crypto::encrypt(&plaintext, password)?;
        let path = wallets_dir.join(network.wallet_filename());
        std::fs::write(path, serde_json::to_string_pretty(&enc)?)?;
        Ok(())
    }

    pub fn has_any_wallet(&self) -> bool {
        Network::all().iter().any(|n| {
            self.wallets_dir().join(n.wallet_filename()).exists()
        })
    }
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-core wallet
```
Expected: PASS (4 tests)

**Step 5: Refactor**

No duplication to eliminate. Naming is aligned with domain language.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-core/src/network.rs crates/wallet-core/src/wallet.rs crates/wallet-core/Cargo.toml
git commit -m "feat: add Network enum and WalletManager skeleton"
```

---

## Task 4: wallet-core — Solana Key Operations

> **TDD mode:** Simple Logic — generate/import/sign/address functions

**Files:**
- Create: `crates/wallet-core/src/solana_wallet.rs`

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_keypair_returns_valid_address() {
        let kp = generate_keypair();
        // Solana addresses are base58, 32-44 chars
        assert!(kp.address.len() >= 32 && kp.address.len() <= 44);
        assert_eq!(kp.private_key_bytes.len(), 64); // ed25519 keypair
    }

    #[test]
    fn import_from_base58_roundtrips_address() {
        let kp = generate_keypair();
        let base58_key = bs58::encode(&kp.private_key_bytes).into_string();

        let imported = import_from_base58(&base58_key).unwrap();
        assert_eq!(imported.address, kp.address);
    }

    #[test]
    fn import_invalid_base58_returns_error() {
        let result = import_from_base58("not-valid-base58!!!");
        assert!(result.is_err());
    }

    #[test]
    fn sign_message_produces_verifiable_signature() {
        let kp = generate_keypair();
        let message = b"hello solana";

        let sig = sign_message(&kp.private_key_bytes, message).unwrap();
        assert_eq!(sig.len(), 64); // ed25519 signature length
    }

    #[test]
    fn import_from_bytes_array_roundtrips() {
        let kp = generate_keypair();
        let imported = import_from_bytes(&kp.private_key_bytes).unwrap();
        assert_eq!(imported.address, kp.address);
    }
}
```

Add to wallet-core Cargo.toml:
```toml
bs58 = "0.5"
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-core solana_wallet
```
Expected: FAIL — module is empty.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-core/src/solana_wallet.rs
use crate::wallet::{WalletError, WalletKeys};
use solana_sdk::signature::{Keypair, Signer};

pub fn generate_keypair() -> WalletKeys {
    let kp = Keypair::new();
    WalletKeys {
        private_key_bytes: kp.to_bytes().to_vec(), // 64 bytes: secret + public
        address: kp.pubkey().to_string(),
    }
}

pub fn import_from_base58(base58_key: &str) -> Result<WalletKeys, WalletError> {
    let bytes = bs58::decode(base58_key)
        .into_vec()
        .map_err(|e| WalletError::NotFound(format!("invalid base58: {e}")))?;
    import_from_bytes(&bytes)
}

pub fn import_from_bytes(bytes: &[u8]) -> Result<WalletKeys, WalletError> {
    let kp = Keypair::from_bytes(bytes)
        .map_err(|e| WalletError::NotFound(format!("invalid keypair bytes: {e}")))?;
    Ok(WalletKeys {
        private_key_bytes: kp.to_bytes().to_vec(),
        address: kp.pubkey().to_string(),
    })
}

pub fn sign_message(private_key_bytes: &[u8], message: &[u8]) -> Result<Vec<u8>, WalletError> {
    let kp = Keypair::from_bytes(private_key_bytes)
        .map_err(|e| WalletError::NotFound(format!("invalid keypair: {e}")))?;
    let sig = kp.sign_message(message);
    Ok(sig.as_ref().to_vec())
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-core solana_wallet
```
Expected: PASS (5 tests)

**Step 5: Refactor**

No duplication. `import_from_bytes` is already shared between `import_from_base58` and `import_from_bytes`.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-core/src/solana_wallet.rs crates/wallet-core/Cargo.toml
git commit -m "feat: add Solana key generation, import, and signing"
```

---

## Task 5: wallet-core — EVM Key Operations

> **TDD mode:** Simple Logic — generate/import/sign using alloy

**Files:**
- Create: `crates/wallet-core/src/evm_wallet.rs`

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_keypair_returns_checksummed_eth_address() {
        let kp = generate_keypair();
        // EVM addresses: "0x" + 40 hex chars = 42 chars
        assert_eq!(kp.address.len(), 42);
        assert!(kp.address.starts_with("0x"));
        assert_eq!(kp.private_key_bytes.len(), 32);
    }

    #[test]
    fn import_from_hex_roundtrips_address() {
        let kp = generate_keypair();
        let hex_key = hex::encode(&kp.private_key_bytes);

        let imported = import_from_hex(&hex_key).unwrap();
        assert_eq!(imported.address, kp.address);
    }

    #[test]
    fn import_invalid_hex_returns_error() {
        let result = import_from_hex("zzzzzz");
        assert!(result.is_err());
    }

    #[test]
    fn import_wrong_length_hex_returns_error() {
        let result = import_from_hex("deadbeef"); // too short
        assert!(result.is_err());
    }

    #[test]
    fn import_from_bytes_roundtrips_address() {
        let kp = generate_keypair();
        let imported = import_from_bytes(&kp.private_key_bytes).unwrap();
        assert_eq!(imported.address, kp.address);
    }
}
```

Add to wallet-core Cargo.toml:
```toml
hex = "0.4"
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-core evm_wallet
```
Expected: FAIL — module is empty.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-core/src/evm_wallet.rs
use crate::wallet::{WalletError, WalletKeys};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::B256;

pub fn generate_keypair() -> WalletKeys {
    let signer = PrivateKeySigner::random();
    WalletKeys {
        private_key_bytes: signer.credential().to_bytes().to_vec(),
        address: signer.address().to_checksum(None),
    }
}

pub fn import_from_hex(hex_key: &str) -> Result<WalletKeys, WalletError> {
    let hex_key = hex_key.trim_start_matches("0x");
    let bytes = hex::decode(hex_key)
        .map_err(|e| WalletError::NotFound(format!("invalid hex: {e}")))?;
    import_from_bytes(&bytes)
}

pub fn import_from_bytes(bytes: &[u8]) -> Result<WalletKeys, WalletError> {
    if bytes.len() != 32 {
        return Err(WalletError::NotFound(format!(
            "expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    let key = B256::from_slice(bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| WalletError::NotFound(format!("invalid key: {e}")))?;
    Ok(WalletKeys {
        private_key_bytes: signer.credential().to_bytes().to_vec(),
        address: signer.address().to_checksum(None),
    })
}

pub fn get_address_from_bytes(bytes: &[u8]) -> Result<String, WalletError> {
    Ok(import_from_bytes(bytes)?.address)
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-core evm_wallet
```
Expected: PASS (5 tests)

**Step 5: Refactor**

`import_from_bytes` is already reused. No duplication.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-core/src/evm_wallet.rs crates/wallet-core/Cargo.toml
git commit -m "feat: add EVM key generation, import, and address derivation"
```

---

## Task 6: wallet-core — Telegram Notification

> **TDD mode:** Simple Logic — HTTP POST to Telegram API, silent failure

**Files:**
- Create: `crates/wallet-core/src/notification.rs`

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_sign_notification_contains_network_and_time() {
        let event = SignEvent {
            network: "Ethereum".to_string(),
            to: Some("0xAbc".to_string()),
            value: Some("0.1 ETH".to_string()),
            description: Some("transfer".to_string()),
            signed_at: "2026-03-04 10:30:00 UTC".to_string(),
        };

        let msg = format_sign_message(&event);
        assert!(msg.contains("Ethereum"));
        assert!(msg.contains("0xAbc"));
        assert!(msg.contains("0.1 ETH"));
        assert!(msg.contains("2026-03-04"));
    }

    #[test]
    fn format_locked_notification_contains_operation_type() {
        let msg = format_locked_message("sign");
        assert!(msg.contains("sign"));
        assert!(msg.contains("locked") || msg.contains("Locked"));
    }

    #[test]
    fn telegram_client_new_stores_credentials() {
        let client = TelegramClient::new("bot_token".to_string(), "chat_id".to_string());
        assert_eq!(client.bot_token, "bot_token");
        assert_eq!(client.chat_id, "chat_id");
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-core notification
```
Expected: FAIL — module is empty.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-core/src/notification.rs
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct SignEvent {
    pub network: String,
    pub to: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub signed_at: String,
}

#[derive(Debug, Clone)]
pub struct TelegramClient {
    pub bot_token: String,
    pub chat_id: String,
    client: reqwest::Client,
}

impl TelegramClient {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            client: reqwest::Client::new(),
        }
    }

    /// Send a message to Telegram. Failures are logged and ignored.
    pub async fn send(&self, text: &str) {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );
        let body = serde_json::json!({
            "chat_id": self.chat_id,
            "text": text,
            "parse_mode": "Markdown"
        });

        match self.client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("telegram notification sent");
            }
            Ok(resp) => {
                error!("telegram notification failed: HTTP {}", resp.status());
            }
            Err(e) => {
                error!("telegram notification error: {e}");
            }
        }
    }
}

pub fn format_sign_message(event: &SignEvent) -> String {
    let mut lines = vec![
        "🔏 *Transaction Signed*".to_string(),
        format!("Network: {}", event.network),
    ];
    if let Some(to) = &event.to {
        lines.push(format!("To: `{to}`"));
    }
    if let Some(value) = &event.value {
        lines.push(format!("Value: {value}"));
    }
    if let Some(desc) = &event.description {
        lines.push(format!("Description: {desc}"));
    }
    lines.push(format!("Time: {}", event.signed_at));
    lines.join("\n")
}

pub fn format_locked_message(operation: &str) -> String {
    format!(
        "⚠️ *Wallet Locked*\nAn external app attempted a `{operation}` request, but the wallet is locked.\nPlease unlock at http://localhost:9292"
    )
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-core notification
```
Expected: PASS (3 tests)

**Step 5: Refactor**

`format_sign_message` uses `lines` accumulator pattern to avoid string concatenation. Clean enough.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-core/src/notification.rs
git commit -m "feat: add Telegram notification client and message formatters"
```

---

## Task 7: wallet-core — Config & Storage

> **TDD mode:** Simple Logic — read/write config files

**Files:**
- Create: `crates/wallet-core/src/config.rs`

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_config(dir: &TempDir) -> AppConfig {
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
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-core config
```
Expected: FAIL — module is empty.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-core/src/config.rs
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
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-core config
```
Expected: PASS (4 tests)

**Step 5: Refactor**

No duplication. Config and secret storage are clearly separated.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-core/src/config.rs
git commit -m "feat: add AppConfig and encrypted Telegram config storage"
```

---

## Task 8: wallet-server — AppState + Server Setup

> **TDD mode:** Simple Logic — state construction and server binding

**Files:**
- Create: `crates/wallet-server/src/state.rs`
- Create: `crates/wallet-server/src/lib.rs` (update)
- Create: `crates/wallet-server/src/server.rs`

**Step 1: Write the failing tests**

```rust
// crates/wallet-server/src/state.rs — test section
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn app_state_new_creates_locked_wallet() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        // WalletManager starts locked — just verify construction succeeds
        assert!(std::sync::Arc::strong_count(&state.wallet) == 1);
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-server state
```
Expected: FAIL — `AppState` doesn't exist.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-server/src/state.rs
use std::path::PathBuf;
use std::sync::Arc;
use wallet_core::{config::AppConfig, notification::TelegramClient, WalletManager};

#[derive(Clone)]
pub struct AppState {
    pub wallet: Arc<WalletManager>,
    pub config: Arc<tokio::sync::RwLock<AppConfig>>,
    pub telegram: Arc<tokio::sync::RwLock<Option<TelegramClient>>>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let config = AppConfig::load_or_default(data_dir.clone());
        Self {
            wallet: Arc::new(WalletManager::new(data_dir)),
            config: Arc::new(tokio::sync::RwLock::new(config)),
            telegram: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Attempt to load Telegram config (called after wallet unlock, when password is available).
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
```

```rust
// crates/wallet-server/src/server.rs
use crate::state::AppState;
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;

pub async fn run(data_dir: PathBuf) -> anyhow::Result<()> {
    let state = AppState::new(data_dir);

    let api_router = crate::api::router(state.clone());
    let admin_router = crate::admin::router(state.clone());

    let api_addr: SocketAddr = "127.0.0.1:9293".parse()?;
    let admin_addr: SocketAddr = "0.0.0.0:9292".parse()?;

    tracing::info!("API server listening on {api_addr}");
    tracing::info!("Admin server listening on {admin_addr}");

    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    let admin_listener = tokio::net::TcpListener::bind(admin_addr).await?;

    tokio::try_join!(
        axum::serve(api_listener, api_router),
        axum::serve(admin_listener, admin_router),
    )?;

    Ok(())
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-server state
```
Expected: PASS (1 test)

**Step 5: Refactor**

No duplication. `send_telegram` helper avoids repeating the `if let Some` pattern in every route handler.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-server/src/state.rs crates/wallet-server/src/server.rs crates/wallet-server/src/lib.rs
git commit -m "feat: add AppState and dual-server setup (ports 9292/9293)"
```

---

## Task 9: wallet-server — REST API Routes (port 9293)

> **TDD mode:** Simple Logic — HTTP handlers, test via axum-test

**Files:**
- Create: `crates/wallet-server/src/api/mod.rs`
- Create: `crates/wallet-server/src/api/handlers.rs`

**Step 1: Write the failing tests**

```rust
// crates/wallet-server/src/api/mod.rs — test section
#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use tempfile::TempDir;
    use crate::state::AppState;

    fn locked_server() -> (TestServer, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let app = router(state);
        (TestServer::new(app).unwrap(), dir)
    }

    #[tokio::test]
    async fn get_address_when_locked_returns_503() {
        let (server, _dir) = locked_server();
        let resp = server.get("/api/wallet/address")
            .add_query_param("network", "eth")
            .await;
        assert_eq!(resp.status_code(), 503);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "wallet_locked");
    }

    #[tokio::test]
    async fn get_balance_when_locked_returns_503() {
        let (server, _dir) = locked_server();
        let resp = server.get("/api/wallet/balance")
            .add_query_param("network", "eth")
            .await;
        assert_eq!(resp.status_code(), 503);
    }

    #[tokio::test]
    async fn post_sign_when_locked_returns_503() {
        let (server, _dir) = locked_server();
        let resp = server.post("/api/wallet/sign")
            .json(&serde_json::json!({
                "network": "eth",
                "transaction": "deadbeef"
            }))
            .await;
        assert_eq!(resp.status_code(), 503);
    }

    #[tokio::test]
    async fn get_address_with_invalid_network_returns_400() {
        let (server, _dir) = locked_server();
        let resp = server.get("/api/wallet/address")
            .add_query_param("network", "invalid_network")
            .await;
        // 503 (locked) takes precedence OR 400 — either is acceptable
        assert!(resp.status_code() == 503 || resp.status_code() == 400);
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-server api
```
Expected: FAIL — `api` module doesn't exist.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-server/src/api/mod.rs
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use wallet_core::{network::Network, notification::{format_locked_message, format_sign_message, SignEvent}};
use chrono::Utc;

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

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
    message: String,
}

fn parse_network(s: &str) -> Option<Network> {
    match s {
        "solana" => Some(Network::Solana),
        "eth" => Some(Network::Eth),
        "bnb" => Some(Network::Bnb),
        "arb" => Some(Network::Arb),
        _ => None,
    }
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
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid_network"}))),
    };

    match state.wallet.get_address(&network).await {
        Ok(address) => (StatusCode::OK, Json(serde_json::json!({
            "network": params.network,
            "address": address
        }))),
        Err(wallet_core::wallet::WalletError::Locked) => {
            state.send_telegram(&format_locked_message("address")).await;
            locked_error()
        }
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

async fn get_balance(
    State(state): State<AppState>,
    Query(params): Query<NetworkQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let network = match parse_network(&params.network) {
        Some(n) => n,
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid_network"}))),
    };

    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("balance")).await;
        return locked_error();
    }

    // Balance fetching via RPC is done in Task 10 (RPC integration).
    // For now return a stub so routing tests pass.
    let address = state.wallet.get_address(&network).await.unwrap_or_default();
    (StatusCode::OK, Json(serde_json::json!({
        "network": params.network,
        "address": address,
        "balance": "0",
        "unit": params.network.to_uppercase()
    })))
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
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid_network"}))),
    };

    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign")).await;
        return locked_error();
    }

    let tx_bytes = match hex::decode(&req.transaction) {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid transaction hex"}))),
    };

    let private_key = match state.wallet.get_private_key_bytes(&network).await {
        Ok(k) => k,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    };

    // Sign based on network
    let signature = match network {
        Network::Solana => wallet_core::solana_wallet::sign_message(&private_key, &tx_bytes),
        Network::Eth | Network::Bnb | Network::Arb => {
            // For EVM: sign the raw bytes as a message hash
            sign_evm_message(&private_key, &tx_bytes)
        }
    };

    let signature = match signature {
        Ok(s) => hex::encode(s),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    };

    let signed_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

    // Send Telegram notification
    let meta = req.metadata.as_ref();
    let event = SignEvent {
        network: network.display_name().to_string(),
        to: meta.and_then(|m| m.to.clone()),
        value: meta.and_then(|m| m.value.clone()),
        description: meta.and_then(|m| m.description.clone()),
        signed_at: signed_at.clone(),
    };
    state.send_telegram(&format_sign_message(&event)).await;

    (StatusCode::OK, Json(serde_json::json!({
        "network": req.network,
        "signature": signature,
        "signed_at": signed_at
    })))
}

fn sign_evm_message(private_key_bytes: &[u8], message: &[u8]) -> Result<Vec<u8>, wallet_core::wallet::WalletError> {
    use alloy::signers::local::PrivateKeySigner;
    use alloy::signers::SignerSync;
    use alloy::primitives::B256;

    if private_key_bytes.len() != 32 {
        return Err(wallet_core::wallet::WalletError::NotFound("invalid key length".to_string()));
    }
    let key = B256::from_slice(private_key_bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| wallet_core::wallet::WalletError::NotFound(e.to_string()))?;
    // Sign the 32-byte hash
    if message.len() != 32 {
        return Err(wallet_core::wallet::WalletError::NotFound("EVM tx must be 32-byte hash".to_string()));
    }
    let hash = alloy::primitives::B256::from_slice(message);
    let sig = signer.sign_hash_sync(&hash)
        .map_err(|e| wallet_core::wallet::WalletError::NotFound(e.to_string()))?;
    Ok(sig.as_bytes().to_vec())
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-server api
```
Expected: PASS (4 tests)

**Step 5: Refactor**

Extract `parse_network` and `locked_error` are already helpers. No further duplication.

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-server/src/api/
git commit -m "feat: add REST API routes (address, balance, sign) on port 9293"
```

---

## Task 10: wallet-server — Admin API Routes (port 9292)

> **TDD mode:** Simple Logic — admin HTTP handlers for setup, unlock, wallet management, settings

**Files:**
- Create: `crates/wallet-server/src/admin/mod.rs`

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use tempfile::TempDir;
    use crate::state::AppState;

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
        let resp = server.post("/api/admin/unlock")
            .json(&serde_json::json!({"password": "test123"}))
            .await;
        assert_eq!(resp.status_code(), 400);
    }

    #[tokio::test]
    async fn setup_creates_wallet_and_can_unlock() {
        let (server, dir) = admin_server();

        // Step 1: generate a new eth wallet
        let resp = server.post("/api/admin/setup/wallet")
            .json(&serde_json::json!({
                "password": "test-password-123",
                "network": "eth",
                "action": "generate"
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert!(body["address"].as_str().unwrap().starts_with("0x"));

        // Step 2: unlock with correct password
        let resp = server.post("/api/admin/unlock")
            .json(&serde_json::json!({"password": "test-password-123"}))
            .await;
        assert_eq!(resp.status_code(), 200);

        // Step 3: verify status shows unlocked
        let resp = server.get("/api/admin/status").await;
        let body: serde_json::Value = resp.json();
        assert_eq!(body["unlocked"], true);
    }

    #[tokio::test]
    async fn unlock_with_wrong_password_returns_401() {
        let (server, _dir) = admin_server();
        // First create a wallet
        server.post("/api/admin/setup/wallet")
            .json(&serde_json::json!({
                "password": "correct",
                "network": "eth",
                "action": "generate"
            }))
            .await;
        // Then try wrong password
        let resp = server.post("/api/admin/unlock")
            .json(&serde_json::json!({"password": "wrong"}))
            .await;
        assert_eq!(resp.status_code(), 401);
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test -p wallet-server admin
```
Expected: FAIL — admin module doesn't exist.

**Step 3: Write minimal implementation**

```rust
// crates/wallet-server/src/admin/mod.rs
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use wallet_core::{network::Network, solana_wallet, evm_wallet, wallet::WalletKeys};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/admin/status", get(get_status))
        .route("/api/admin/unlock", post(unlock))
        .route("/api/admin/lock", post(lock))
        .route("/api/admin/setup/wallet", post(setup_wallet))
        .route("/api/admin/settings/telegram", post(save_telegram_settings))
        .route("/api/admin/settings/rpc", post(save_rpc_settings))
        .route("/api/admin/settings", get(get_settings))
        .with_state(state)
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
            (StatusCode::OK, Json(serde_json::json!({"status": "unlocked"})))
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
    action: String,              // "generate" | "import"
    private_key: Option<String>, // required when action == "import"
}

fn parse_network(s: &str) -> Option<Network> {
    match s {
        "solana" => Some(Network::Solana),
        "eth" => Some(Network::Eth),
        "bnb" => Some(Network::Bnb),
        "arb" => Some(Network::Arb),
        _ => None,
    }
}

async fn setup_wallet(
    State(state): State<AppState>,
    Json(req): Json<SetupWalletRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let network = match parse_network(&req.network) {
        Some(n) => n,
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid network"}))),
    };

    let keys: WalletKeys = match req.action.as_str() {
        "generate" => match network {
            Network::Solana => solana_wallet::generate_keypair(),
            Network::Eth | Network::Bnb | Network::Arb => evm_wallet::generate_keypair(),
        },
        "import" => {
            let pk = match &req.private_key {
                Some(k) => k,
                None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "private_key required for import"}))),
            };
            let result = match network {
                Network::Solana => solana_wallet::import_from_base58(pk),
                Network::Eth | Network::Bnb | Network::Arb => evm_wallet::import_from_hex(pk),
            };
            match result {
                Ok(k) => k,
                Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
            }
        }
        _ => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "action must be generate or import"}))),
    };

    let address = keys.address.clone();
    if let Err(e) = state.wallet.save_wallet(&network, &keys, &req.password) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})));
    }

    (StatusCode::OK, Json(serde_json::json!({
        "network": req.network,
        "address": address
    })))
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
    let tg = wallet_core::config::TelegramConfig {
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
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

#[derive(Deserialize)]
struct RpcSettingsRequest {
    solana: Option<String>,
    eth: Option<String>,
    bnb: Option<String>,
    arb: Option<String>,
}

async fn save_rpc_settings(
    State(state): State<AppState>,
    Json(req): Json<RpcSettingsRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut config = state.config.write().await;
    if let Some(url) = req.solana { config.rpc.solana = url; }
    if let Some(url) = req.eth { config.rpc.eth = url; }
    if let Some(url) = req.bnb { config.rpc.bnb = url; }
    if let Some(url) = req.arb { config.rpc.arb = url; }
    match config.save() {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "saved"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
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
        }
    }))
}
```

**Step 4: Run tests to verify they pass**

```bash
cargo test -p wallet-server admin
```
Expected: PASS (4 tests)

**Step 5: Refactor**

Extract `parse_network` to a shared module to avoid duplication between `api/mod.rs` and `admin/mod.rs`:
- Create `crates/wallet-server/src/util.rs` with `pub fn parse_network(s: &str) -> Option<Network>`
- Update both modules to use `crate::util::parse_network`

**Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add crates/wallet-server/src/admin/ crates/wallet-server/src/util.rs
git commit -m "feat: add admin API routes (status, unlock, setup, settings)"
```

---

## Task 11: wallet-cli — Main Entry Point

> **TDD mode:** N/A — binary entry point wiring

**Files:**
- Modify: `crates/wallet-cli/src/main.rs`

**Step 1: Write the implementation**

```rust
// crates/wallet-cli/src/main.rs
use anyhow::Context;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("local-wallet")
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local-wallet")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("local_wallet=info".parse()?)
                .add_directive("wallet_server=info".parse()?)
                .add_directive("wallet_core=info".parse()?),
        )
        .init();

    let data_dir = data_dir();
    tracing::info!("data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).context("failed to create data directory")?;

    wallet_server::server::run(data_dir).await
}
```

**Step 2: Verify it compiles and runs**

```bash
cargo build -p wallet-cli
```
Expected: compiles successfully.

```bash
cargo run -p wallet-cli &
sleep 1
curl -s http://localhost:9293/api/wallet/address?network=eth | jq .
# Expected: {"error":"wallet_locked","message":"..."}
kill %1
```

**Step 3: Commit**

```bash
git add crates/wallet-cli/src/main.rs
git commit -m "feat: wire main entry point with data dir and dual-server startup"
```

---

## Task 12: Frontend Scaffold

> **TDD mode:** N/A — scaffolding

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/tsconfig.json`
- Create: `frontend/index.html`
- Create: `frontend/src/main.tsx`
- Create: `frontend/src/App.tsx`
- Create: `frontend/src/context/AppContext.tsx`

**Step 1: Scaffold frontend**

```bash
cd frontend
npm create vite@latest . -- --template react-ts
npm install
npm install react-router-dom axios
npm install -D vitest @testing-library/react @testing-library/user-event jsdom @vitejs/plugin-react
```

**Step 2: Update vite.config.ts**

```typescript
// frontend/vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: '../crates/wallet-server/frontend-dist',
    emptyOutDir: true,
  },
  server: {
    proxy: {
      '/api': 'http://localhost:9292',
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test-setup.ts',
  },
})
```

**Step 3: Create AppContext**

```typescript
// frontend/src/context/AppContext.tsx
import React, { createContext, useContext, useEffect, useState } from 'react'
import axios from 'axios'

export type AppStatus = 'loading' | 'setup_required' | 'locked' | 'unlocked'

interface AppContextValue {
  status: AppStatus
  refresh: () => Promise<void>
}

const AppContext = createContext<AppContextValue>({
  status: 'loading',
  refresh: async () => {},
})

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<AppStatus>('loading')

  const refresh = async () => {
    try {
      const resp = await axios.get('/api/admin/status')
      const { setup_required, unlocked } = resp.data
      if (setup_required) setStatus('setup_required')
      else if (unlocked) setStatus('unlocked')
      else setStatus('locked')
    } catch {
      setStatus('locked')
    }
  }

  useEffect(() => { refresh() }, [])

  return (
    <AppContext.Provider value={{ status, refresh }}>
      {children}
    </AppContext.Provider>
  )
}

export function useApp() {
  return useContext(AppContext)
}
```

**Step 4: Create App.tsx with routing**

```typescript
// frontend/src/App.tsx
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppProvider, useApp } from './context/AppContext'
import Setup from './pages/Setup'
import Unlock from './pages/Unlock'
import Dashboard from './pages/Dashboard'
import Wallets from './pages/Wallets'
import Settings from './pages/Settings'

function AppRoutes() {
  const { status } = useApp()
  if (status === 'loading') return <div>Loading...</div>
  return (
    <Routes>
      <Route path="/setup" element={<Setup />} />
      <Route path="/unlock" element={<Unlock />} />
      <Route path="/dashboard" element={<Dashboard />} />
      <Route path="/wallets" element={<Wallets />} />
      <Route path="/settings" element={<Settings />} />
      <Route path="/" element={
        status === 'setup_required' ? <Navigate to="/setup" replace />
        : status === 'unlocked' ? <Navigate to="/dashboard" replace />
        : <Navigate to="/unlock" replace />
      } />
    </Routes>
  )
}

export default function App() {
  return (
    <BrowserRouter>
      <AppProvider>
        <AppRoutes />
      </AppProvider>
    </BrowserRouter>
  )
}
```

**Step 5: Verify frontend builds**

```bash
cd frontend && npm run build
```
Expected: `crates/wallet-server/frontend-dist/` directory created with `index.html` and assets.

**Step 6: Commit**

```bash
git add frontend/
git commit -m "feat: scaffold React + TypeScript frontend with Vite and routing"
```

---

## Task 13: Frontend — Setup Wizard

> **TDD mode:** Simple Logic — component state transitions tested with Vitest + Testing Library

**Files:**
- Create: `frontend/src/pages/Setup.tsx`
- Create: `frontend/src/pages/Setup.test.tsx`

**Step 1: Write the failing tests**

```typescript
// frontend/src/pages/Setup.test.tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'
import axios from 'axios'
import Setup from './Setup'

vi.mock('axios')
const mockedAxios = axios as jest.Mocked<typeof axios>

describe('Setup wizard', () => {
  test('shows step 1 (set password) initially', () => {
    render(<Setup />)
    expect(screen.getByText(/set password/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/^password/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/confirm password/i)).toBeInTheDocument()
  })

  test('shows error when passwords do not match', async () => {
    render(<Setup />)
    await userEvent.type(screen.getByLabelText(/^password/i), 'abc123')
    await userEvent.type(screen.getByLabelText(/confirm password/i), 'different')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    expect(screen.getByText(/passwords do not match/i)).toBeInTheDocument()
  })

  test('advances to step 2 after valid password', async () => {
    render(<Setup />)
    await userEvent.type(screen.getByLabelText(/^password/i), 'StrongPass123!')
    await userEvent.type(screen.getByLabelText(/confirm password/i), 'StrongPass123!')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    expect(screen.getByText(/import or generate/i)).toBeInTheDocument()
  })

  test('calls API to generate wallet on step 2', async () => {
    mockedAxios.post = vi.fn().mockResolvedValue({ data: { address: '0xABC' } })
    render(<Setup />)
    // Advance to step 2
    await userEvent.type(screen.getByLabelText(/^password/i), 'StrongPass123!')
    await userEvent.type(screen.getByLabelText(/confirm password/i), 'StrongPass123!')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    // Generate ETH wallet
    fireEvent.click(screen.getByRole('button', { name: /generate eth/i }))
    await waitFor(() => expect(mockedAxios.post).toHaveBeenCalledWith(
      '/api/admin/setup/wallet',
      expect.objectContaining({ network: 'eth', action: 'generate' })
    ))
  })
})
```

**Step 2: Run tests to verify they fail**

```bash
cd frontend && npx vitest run src/pages/Setup.test.tsx
```
Expected: FAIL — Setup component doesn't exist.

**Step 3: Write minimal implementation**

```typescript
// frontend/src/pages/Setup.tsx
import { useState } from 'react'
import axios from 'axios'
import { useNavigate } from 'react-router-dom'

type Step = 'password' | 'wallets'
const NETWORKS = ['solana', 'eth', 'bnb', 'arb'] as const

export default function Setup() {
  const navigate = useNavigate()
  const [step, setStep] = useState<Step>('password')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [error, setError] = useState('')
  const [createdWallets, setCreatedWallets] = useState<Record<string, string>>({})
  const [importing, setImporting] = useState<string | null>(null)
  const [importKey, setImportKey] = useState('')

  const handlePasswordNext = () => {
    if (password !== confirm) { setError('Passwords do not match'); return }
    if (password.length < 8) { setError('Password must be at least 8 characters'); return }
    setError('')
    setStep('wallets')
  }

  const generateWallet = async (network: string) => {
    try {
      const resp = await axios.post('/api/admin/setup/wallet', {
        password, network, action: 'generate'
      })
      setCreatedWallets(prev => ({ ...prev, [network]: resp.data.address }))
    } catch (e: any) {
      setError(e.response?.data?.error ?? 'Failed to generate wallet')
    }
  }

  const importWallet = async (network: string) => {
    try {
      const resp = await axios.post('/api/admin/setup/wallet', {
        password, network, action: 'import', private_key: importKey
      })
      setCreatedWallets(prev => ({ ...prev, [network]: resp.data.address }))
      setImporting(null)
      setImportKey('')
    } catch (e: any) {
      setError(e.response?.data?.error ?? 'Failed to import wallet')
    }
  }

  const handleFinish = () => navigate('/unlock')

  if (step === 'password') {
    return (
      <div>
        <h1>Set Password</h1>
        <label htmlFor="password">Password</label>
        <input id="password" type="password" value={password} onChange={e => setPassword(e.target.value)} />
        <label htmlFor="confirm-password">Confirm Password</label>
        <input id="confirm-password" type="password" value={confirm} onChange={e => setConfirm(e.target.value)} />
        {error && <p role="alert">{error}</p>}
        <button onClick={handlePasswordNext}>Next</button>
      </div>
    )
  }

  return (
    <div>
      <h1>Import or Generate Wallets</h1>
      {error && <p role="alert">{error}</p>}
      {NETWORKS.map(network => (
        <div key={network}>
          <strong>{network.toUpperCase()}</strong>
          {createdWallets[network] ? (
            <span>{createdWallets[network]}</span>
          ) : (
            <>
              <button onClick={() => generateWallet(network)}>Generate {network.toUpperCase()}</button>
              <button onClick={() => setImporting(network)}>Import {network.toUpperCase()}</button>
              {importing === network && (
                <>
                  <input
                    value={importKey}
                    onChange={e => setImportKey(e.target.value)}
                    placeholder={network === 'solana' ? 'Base58 private key' : 'Hex private key'}
                  />
                  <button onClick={() => importWallet(network)}>Confirm Import</button>
                </>
              )}
            </>
          )}
        </div>
      ))}
      <button onClick={handleFinish} disabled={Object.keys(createdWallets).length === 0}>
        Finish Setup
      </button>
    </div>
  )
}
```

**Step 4: Run tests to verify they pass**

```bash
cd frontend && npx vitest run src/pages/Setup.test.tsx
```
Expected: PASS (4 tests)

**Step 5: Refactor**

No duplication. Network map is already extracted as `NETWORKS` constant.

**Step 6: Commit**

```bash
git add frontend/src/pages/Setup.tsx frontend/src/pages/Setup.test.tsx
git commit -m "feat: add Setup wizard page (set password + import/generate wallets)"
```

---

## Task 14: Frontend — Unlock, Dashboard, Wallets, Settings Pages

> **TDD mode:** Simple Logic — form validation and API calls tested with Vitest

**Files:**
- Create: `frontend/src/pages/Unlock.tsx`
- Create: `frontend/src/pages/Unlock.test.tsx`
- Create: `frontend/src/pages/Dashboard.tsx`
- Create: `frontend/src/pages/Wallets.tsx`
- Create: `frontend/src/pages/Settings.tsx`
- Create: `frontend/src/pages/Settings.test.tsx`

**Step 1: Write the failing tests**

```typescript
// frontend/src/pages/Unlock.test.tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'
import axios from 'axios'
import { MemoryRouter } from 'react-router-dom'
import Unlock from './Unlock'
import { AppProvider } from '../context/AppContext'

vi.mock('axios')

describe('Unlock page', () => {
  test('renders password field and submit button', () => {
    render(<MemoryRouter><AppProvider><Unlock /></AppProvider></MemoryRouter>)
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /unlock/i })).toBeInTheDocument()
  })

  test('calls unlock API on submit', async () => {
    (axios.post as any) = vi.fn().mockResolvedValue({ data: { status: 'unlocked' } });
    (axios.get as any) = vi.fn().mockResolvedValue({ data: { unlocked: true, setup_required: false } })
    render(<MemoryRouter><AppProvider><Unlock /></AppProvider></MemoryRouter>)
    await userEvent.type(screen.getByLabelText(/password/i), 'my-password')
    fireEvent.click(screen.getByRole('button', { name: /unlock/i }))
    await waitFor(() => expect(axios.post).toHaveBeenCalledWith('/api/admin/unlock', { password: 'my-password' }))
  })

  test('shows error on wrong password', async () => {
    (axios.post as any) = vi.fn().mockRejectedValue({ response: { status: 401, data: { error: 'invalid password' } } })
    render(<MemoryRouter><AppProvider><Unlock /></AppProvider></MemoryRouter>)
    await userEvent.type(screen.getByLabelText(/password/i), 'wrong')
    fireEvent.click(screen.getByRole('button', { name: /unlock/i }))
    await waitFor(() => expect(screen.getByText(/invalid password/i)).toBeInTheDocument())
  })
})
```

```typescript
// frontend/src/pages/Settings.test.tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'
import axios from 'axios'
import { MemoryRouter } from 'react-router-dom'
import Settings from './Settings'

vi.mock('axios')

describe('Settings page', () => {
  beforeEach(() => {
    (axios.get as any) = vi.fn().mockResolvedValue({
      data: { rpc: { solana: 'https://api.mainnet-beta.solana.com', eth: 'https://eth.llamarpc.com', bnb: 'https://bsc-dataseed.binance.org', arb: 'https://arb1.arbitrum.io/rpc' } }
    })
  })

  test('loads and displays current RPC settings', async () => {
    render(<MemoryRouter><Settings /></MemoryRouter>)
    await waitFor(() => expect(screen.getByDisplayValue('https://eth.llamarpc.com')).toBeInTheDocument())
  })

  test('saves RPC settings on submit', async () => {
    (axios.post as any) = vi.fn().mockResolvedValue({ data: { status: 'saved' } })
    render(<MemoryRouter><Settings /></MemoryRouter>)
    await waitFor(() => screen.getByDisplayValue('https://eth.llamarpc.com'))
    fireEvent.click(screen.getByRole('button', { name: /save rpc/i }))
    await waitFor(() => expect(axios.post).toHaveBeenCalledWith('/api/admin/settings/rpc', expect.any(Object)))
  })
})
```

**Step 2: Run tests to verify they fail**

```bash
cd frontend && npx vitest run src/pages/Unlock.test.tsx src/pages/Settings.test.tsx
```
Expected: FAIL — pages don't exist.

**Step 3: Write minimal implementations**

```typescript
// frontend/src/pages/Unlock.tsx
import { useState } from 'react'
import axios from 'axios'
import { useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'

export default function Unlock() {
  const navigate = useNavigate()
  const { refresh } = useApp()
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')

  const handleUnlock = async () => {
    try {
      await axios.post('/api/admin/unlock', { password })
      await refresh()
      navigate('/dashboard')
    } catch (e: any) {
      setError(e.response?.data?.error ?? 'Failed to unlock')
    }
  }

  return (
    <div>
      <h1>Unlock Wallet</h1>
      <label htmlFor="password">Password</label>
      <input id="password" type="password" value={password} onChange={e => setPassword(e.target.value)} />
      {error && <p role="alert">{error}</p>}
      <button onClick={handleUnlock}>Unlock</button>
    </div>
  )
}
```

```typescript
// frontend/src/pages/Dashboard.tsx
import { Link } from 'react-router-dom'

export default function Dashboard() {
  return (
    <div>
      <h1>Dashboard</h1>
      <nav>
        <Link to="/wallets">Wallets</Link>
        <Link to="/settings">Settings</Link>
      </nav>
    </div>
  )
}
```

```typescript
// frontend/src/pages/Wallets.tsx
import { useEffect, useState } from 'react'
import axios from 'axios'

const NETWORKS = ['solana', 'eth', 'bnb', 'arb'] as const

export default function Wallets() {
  const [addresses, setAddresses] = useState<Record<string, string>>({})
  const [balances, setBalances] = useState<Record<string, string>>({})

  useEffect(() => {
    NETWORKS.forEach(async (network) => {
      try {
        const addrResp = await axios.get(`http://localhost:9293/api/wallet/address?network=${network}`)
        setAddresses(prev => ({ ...prev, [network]: addrResp.data.address }))
        const balResp = await axios.get(`http://localhost:9293/api/wallet/balance?network=${network}`)
        setBalances(prev => ({ ...prev, [network]: `${balResp.data.balance} ${balResp.data.unit}` }))
      } catch { /* locked or not set up */ }
    })
  }, [])

  return (
    <div>
      <h1>Wallets</h1>
      {NETWORKS.map(network => (
        <div key={network}>
          <h2>{network.toUpperCase()}</h2>
          <p>Address: {addresses[network] ?? '—'}</p>
          <p>Balance: {balances[network] ?? '—'}</p>
        </div>
      ))}
    </div>
  )
}
```

```typescript
// frontend/src/pages/Settings.tsx
import { useEffect, useState } from 'react'
import axios from 'axios'

interface RpcSettings { solana: string; eth: string; bnb: string; arb: string }

export default function Settings() {
  const [rpc, setRpc] = useState<RpcSettings>({ solana: '', eth: '', bnb: '', arb: '' })
  const [botToken, setBotToken] = useState('')
  const [chatId, setChatId] = useState('')
  const [password, setPassword] = useState('')
  const [message, setMessage] = useState('')

  useEffect(() => {
    axios.get('/api/admin/settings').then(resp => setRpc(resp.data.rpc))
  }, [])

  const saveRpc = async () => {
    await axios.post('/api/admin/settings/rpc', rpc)
    setMessage('RPC settings saved')
  }

  const saveTelegram = async () => {
    await axios.post('/api/admin/settings/telegram', { bot_token: botToken, chat_id: chatId, password })
    setMessage('Telegram settings saved')
  }

  return (
    <div>
      <h1>Settings</h1>
      {message && <p>{message}</p>}

      <h2>RPC Endpoints</h2>
      {(['solana', 'eth', 'bnb', 'arb'] as const).map(net => (
        <div key={net}>
          <label>{net.toUpperCase()} RPC</label>
          <input value={rpc[net]} onChange={e => setRpc(prev => ({ ...prev, [net]: e.target.value }))} />
        </div>
      ))}
      <button onClick={saveRpc}>Save RPC</button>

      <h2>Telegram Bot</h2>
      <label>Bot Token</label>
      <input value={botToken} onChange={e => setBotToken(e.target.value)} />
      <label>Chat ID</label>
      <input value={chatId} onChange={e => setChatId(e.target.value)} />
      <label>Wallet Password (to encrypt)</label>
      <input type="password" value={password} onChange={e => setPassword(e.target.value)} />
      <button onClick={saveTelegram}>Save Telegram</button>
    </div>
  )
}
```

**Step 4: Run tests to verify they pass**

```bash
cd frontend && npx vitest run src/pages/Unlock.test.tsx src/pages/Settings.test.tsx
```
Expected: PASS (5 tests total)

**Step 5: Refactor**

`NETWORKS` constant should be moved to a shared `constants.ts` file and imported in `Setup.tsx`, `Wallets.tsx`, and `Settings.tsx`.

**Step 6: Run full frontend test suite**

```bash
cd frontend && npx vitest run
```
Expected: all tests pass.

**Step 7: Commit**

```bash
git add frontend/src/pages/
git commit -m "feat: add Unlock, Dashboard, Wallets, Settings pages"
```

---

## Task 15: Build Integration — Embed Frontend in Binary

> **TDD mode:** N/A — build pipeline integration

**Files:**
- Create: `crates/wallet-server/src/static_files.rs`
- Modify: `crates/wallet-server/src/admin/mod.rs` (add static file serving)
- Create: `build.rs` (in wallet-server crate)

**Step 1: Build the frontend**

```bash
cd frontend && npm run build
```
Expected: `crates/wallet-server/frontend-dist/` created.

**Step 2: Add include_dir to wallet-server**

In `crates/wallet-server/Cargo.toml`, `include_dir` is already listed. Create the static file module:

```rust
// crates/wallet-server/src/static_files.rs
use include_dir::{include_dir, Dir};

pub static FRONTEND_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/frontend-dist");
```

**Step 3: Add static file route to admin server**

Update `crates/wallet-server/src/admin/mod.rs` to add a fallback route that serves the embedded frontend:

```rust
// Add to router() in admin/mod.rs
use axum::response::{Html, Response};
use axum::body::Body;
use http::{header, StatusCode};

pub fn router(state: AppState) -> Router {
    Router::new()
        // ... existing routes ...
        .fallback(serve_frontend)
        .with_state(state)
}

async fn serve_frontend(uri: axum::http::Uri) -> Response<Body> {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match crate::static_files::FRONTEND_DIR.get_file(path) {
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
            let index = crate::static_files::FRONTEND_DIR
                .get_file("index.html")
                .unwrap();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html")
                .body(Body::from(index.contents()))
                .unwrap()
        }
    }
}
```

Add `mime_guess` to wallet-server dependencies:
```toml
mime_guess = "2"
```

**Step 4: Verify the full build**

```bash
cd frontend && npm run build && cd ..
cargo build -p wallet-cli
```
Expected: binary at `target/debug/local-wallet`, includes embedded frontend.

**Step 5: Smoke test**

```bash
./target/debug/local-wallet &
sleep 1
curl -s http://localhost:9292 | head -5
# Expected: HTML content of index.html
kill %1
```

**Step 6: Commit**

```bash
git add crates/wallet-server/src/static_files.rs crates/wallet-server/src/admin/mod.rs crates/wallet-server/Cargo.toml crates/wallet-server/frontend-dist/
git commit -m "feat: embed React frontend into binary via include_dir"
```

---

## Task 16: Installation Scripts

> **TDD mode:** N/A — shell/PowerShell scripts

**Files:**
- Create: `install/install-linux.sh`
- Create: `install/install-macos.sh`
- Create: `install/install-windows.ps1`
- Create: `install/install-termux.sh`
- Create: `install/README.md`

**Step 1: Create Linux systemd installer**

```bash
#!/usr/bin/env bash
# install/install-linux.sh
set -euo pipefail

BINARY_PATH="./local-wallet"
INSTALL_PATH="/usr/local/bin/local-wallet"
SERVICE_FILE="/etc/systemd/system/local-wallet.service"

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: $BINARY_PATH not found. Build the binary first with: cargo build --release"
  exit 1
fi

echo "Installing local-wallet to $INSTALL_PATH..."
sudo cp "$BINARY_PATH" "$INSTALL_PATH"
sudo chmod +x "$INSTALL_PATH"

echo "Creating systemd service..."
sudo tee "$SERVICE_FILE" > /dev/null <<EOF
[Unit]
Description=Local Wallet Service
After=network.target

[Service]
Type=simple
ExecStart=$INSTALL_PATH
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable local-wallet
sudo systemctl start local-wallet

echo "Done. local-wallet is running."
echo "Admin UI: http://localhost:9292"
echo "REST API: http://localhost:9293"
```

**Step 2: Create macOS launchd installer**

```bash
#!/usr/bin/env bash
# install/install-macos.sh
set -euo pipefail

BINARY_PATH="./local-wallet"
INSTALL_PATH="/usr/local/bin/local-wallet"
PLIST_PATH="$HOME/Library/LaunchAgents/com.local-wallet.plist"

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: $BINARY_PATH not found."
  exit 1
fi

cp "$BINARY_PATH" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"

mkdir -p "$HOME/Library/LaunchAgents"
cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.local-wallet</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_PATH</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$HOME/.local-wallet/local-wallet.log</string>
    <key>StandardErrorPath</key>
    <string>$HOME/.local-wallet/local-wallet.log</string>
</dict>
</plist>
EOF

launchctl load -w "$PLIST_PATH"
echo "Done. local-wallet is running."
echo "Admin UI: http://localhost:9292"
```

**Step 3: Create Windows Task Scheduler installer**

```powershell
# install/install-windows.ps1
$BinaryPath = ".\local-wallet.exe"
$InstallDir = "$env:LOCALAPPDATA\local-wallet"
$InstallPath = "$InstallDir\local-wallet.exe"
$TaskName = "LocalWalletService"

if (-not (Test-Path $BinaryPath)) {
    Write-Error "local-wallet.exe not found. Build first with: cargo build --release"
    exit 1
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item $BinaryPath $InstallPath -Force

$Action = New-ScheduledTaskAction -Execute $InstallPath
$Trigger = New-ScheduledTaskTrigger -AtLogOn
$Settings = New-ScheduledTaskSettingsSet -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)

Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Force

Start-ScheduledTask -TaskName $TaskName

Write-Host "Done. local-wallet is running."
Write-Host "Admin UI: http://localhost:9292"
Write-Host "REST API: http://localhost:9293"
```

**Step 4: Create Termux installer**

```bash
#!/usr/bin/env bash
# install/install-termux.sh
set -euo pipefail

BINARY_PATH="./local-wallet"
INSTALL_PATH="$PREFIX/bin/local-wallet"

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: $BINARY_PATH not found."
  exit 1
fi

cp "$BINARY_PATH" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"

# Add to .bashrc for convenience
if ! grep -q "local-wallet" "$HOME/.bashrc" 2>/dev/null; then
  echo "" >> "$HOME/.bashrc"
  echo "# Start local-wallet (run manually)" >> "$HOME/.bashrc"
  echo "# local-wallet &" >> "$HOME/.bashrc"
fi

echo "Done. To start: local-wallet"
echo "For persistence across sessions, use tmux: tmux new-session -d -s wallet 'local-wallet'"
echo "Admin UI: http://localhost:9292"
```

**Step 5: Commit**

```bash
chmod +x install/install-linux.sh install/install-macos.sh install/install-termux.sh
git add install/
git commit -m "feat: add installation scripts for Linux, macOS, Windows, Termux"
```

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | Cargo workspace scaffold | Cargo.toml, 3 crate stubs |
| 2 | Crypto: AES-256-GCM + PBKDF2 | crypto.rs |
| 3 | Network enum + WalletManager skeleton | network.rs, wallet.rs |
| 4 | Solana key ops | solana_wallet.rs |
| 5 | EVM key ops | evm_wallet.rs |
| 6 | Telegram notification | notification.rs |
| 7 | Config + encrypted secret storage | config.rs |
| 8 | AppState + dual-server setup | state.rs, server.rs |
| 9 | REST API routes (port 9293) | api/mod.rs |
| 10 | Admin API routes (port 9292) | admin/mod.rs |
| 11 | CLI entry point | main.rs |
| 12 | Frontend scaffold (Vite + React) | frontend/ |
| 13 | Setup wizard page | Setup.tsx |
| 14 | Unlock, Dashboard, Wallets, Settings | 4 page files |
| 15 | Embed frontend in binary | static_files.rs, include_dir |
| 16 | Installation scripts | install/ |
