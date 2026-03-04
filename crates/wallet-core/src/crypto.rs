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
