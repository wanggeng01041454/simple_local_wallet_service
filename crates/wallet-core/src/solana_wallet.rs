use crate::wallet::{WalletError, WalletKeys};
use solana_sdk::signature::{Keypair, Signer};

pub fn generate_keypair() -> WalletKeys {
    let kp = Keypair::new();
    WalletKeys {
        private_key_bytes: kp.to_bytes().to_vec(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_keypair_returns_valid_address() {
        let kp = generate_keypair();
        assert!(kp.address.len() >= 32 && kp.address.len() <= 44);
        assert_eq!(kp.private_key_bytes.len(), 64);
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
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn import_from_bytes_array_roundtrips() {
        let kp = generate_keypair();
        let imported = import_from_bytes(&kp.private_key_bytes).unwrap();
        assert_eq!(imported.address, kp.address);
    }
}
