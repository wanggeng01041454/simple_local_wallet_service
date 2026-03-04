use crate::wallet::{WalletError, WalletKeys};
use alloy::primitives::B256;
use alloy::signers::local::PrivateKeySigner;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_keypair_returns_checksummed_eth_address() {
        let kp = generate_keypair();
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
        let result = import_from_hex("deadbeef");
        assert!(result.is_err());
    }

    #[test]
    fn import_from_bytes_roundtrips_address() {
        let kp = generate_keypair();
        let imported = import_from_bytes(&kp.private_key_bytes).unwrap();
        assert_eq!(imported.address, kp.address);
    }
}
