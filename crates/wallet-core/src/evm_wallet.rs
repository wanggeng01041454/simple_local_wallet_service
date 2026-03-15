use crate::wallet::{WalletError, WalletKeys};
use alloy::consensus::{SignableTransaction, TxEnvelope};
use alloy::eips::eip2718::{Decodable2718, Encodable2718};
use alloy::primitives::{eip191_hash_message, B256, TxKind, U256};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;

pub fn generate_keypair() -> WalletKeys {
    let signer = PrivateKeySigner::random();
    WalletKeys {
        private_key_bytes: signer.credential().to_bytes().to_vec(),
        address: signer.address().to_checksum(None),
    }
}

pub fn import_from_hex(hex_key: &str) -> Result<WalletKeys, WalletError> {
    let hex_key = hex_key.trim_start_matches("0x");
    let bytes =
        hex::decode(hex_key).map_err(|e| WalletError::NotFound(format!("invalid hex: {e}")))?;
    import_from_bytes(&bytes)
}

/// 对 EVM 交易进行签名。
///
/// 返回 (signed_tx_bytes, from_address, to_address, value_wei_decimal)。
pub fn sign_transaction(
    private_key_bytes: &[u8],
    tx_bytes: &[u8],
    chain_id: u64,
) -> Result<(Vec<u8>, String, Option<String>, String), WalletError> {
    if private_key_bytes.len() != 32 {
        return Err(WalletError::InvalidTransaction("key must be 32 bytes".to_string()));
    }
    let key = B256::from_slice(private_key_bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
    let from_address = signer.address().to_checksum(None);

    if tx_bytes.is_empty() {
        return Err(WalletError::InvalidTransaction("empty transaction".to_string()));
    }
    // type 3 (EIP-4844) not supported
    if tx_bytes[0] == 0x03 {
        return Err(WalletError::UnsupportedTxType(3));
    }
    // other unknown typed envelopes
    if tx_bytes[0] != 0x01 && tx_bytes[0] != 0x02 && tx_bytes[0] < 0x80 {
        return Err(WalletError::UnsupportedTxType(tx_bytes[0]));
    }

    let mut buf = tx_bytes;
    let envelope = TxEnvelope::decode_2718(&mut buf)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    match envelope {
        TxEnvelope::Legacy(signed) => {
            let sig = signed.signature();
            if sig.r() != U256::ZERO || sig.s() != U256::ZERO {
                return Err(WalletError::AlreadySigned);
            }
            let tx = signed.tx().clone();
            match tx.chain_id {
                Some(id) if id != chain_id => {
                    return Err(WalletError::ChainIdMismatch { expected: chain_id, actual: id });
                }
                _ => {}
            }
            let mut tx_owned = tx;
            tx_owned.chain_id = Some(chain_id);
            let to = match &tx_owned.to {
                TxKind::Call(addr) => Some(addr.to_checksum(None)),
                TxKind::Create => None,
            };
            let value = tx_owned.value.to_string();
            let hash = tx_owned.signature_hash();
            let new_sig = signer
                .sign_hash_sync(&hash)
                .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
            let new_signed = tx_owned.into_signed(new_sig);
            let mut out = Vec::new();
            TxEnvelope::Legacy(new_signed).encode_2718(&mut out);
            Ok((out, from_address, to, value))
        }

        TxEnvelope::Eip2930(signed) => {
            let sig = signed.signature();
            if sig.r() != U256::ZERO || sig.s() != U256::ZERO {
                return Err(WalletError::AlreadySigned);
            }
            let tx = signed.tx().clone();
            if tx.chain_id != chain_id {
                return Err(WalletError::ChainIdMismatch {
                    expected: chain_id,
                    actual: tx.chain_id,
                });
            }
            let to = match &tx.to {
                TxKind::Call(addr) => Some(addr.to_checksum(None)),
                TxKind::Create => None,
            };
            let value = tx.value.to_string();
            let hash = tx.signature_hash();
            let new_sig = signer
                .sign_hash_sync(&hash)
                .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
            let new_signed = tx.into_signed(new_sig);
            let mut out = Vec::new();
            TxEnvelope::Eip2930(new_signed).encode_2718(&mut out);
            Ok((out, from_address, to, value))
        }

        TxEnvelope::Eip1559(signed) => {
            let sig = signed.signature();
            if sig.r() != U256::ZERO || sig.s() != U256::ZERO {
                return Err(WalletError::AlreadySigned);
            }
            let tx = signed.tx().clone();
            if tx.chain_id != chain_id {
                return Err(WalletError::ChainIdMismatch {
                    expected: chain_id,
                    actual: tx.chain_id,
                });
            }
            let to = match &tx.to {
                TxKind::Call(addr) => Some(addr.to_checksum(None)),
                TxKind::Create => None,
            };
            let value = tx.value.to_string();
            let hash = tx.signature_hash();
            let new_sig = signer
                .sign_hash_sync(&hash)
                .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
            let new_signed = tx.into_signed(new_sig);
            let mut out = Vec::new();
            TxEnvelope::Eip1559(new_signed).encode_2718(&mut out);
            Ok((out, from_address, to, value))
        }

        TxEnvelope::Eip4844(_) => Err(WalletError::UnsupportedTxType(3)),

        _ => Err(WalletError::UnsupportedTxType(tx_bytes[0])),
    }
}

/// 对 EIP-712 typed data 进行签名。
///
/// 返回 (signature_hex, r_hex, s_hex, v_hex)，均含 0x 前缀。
pub fn sign_typed_data(
    private_key_bytes: &[u8],
    typed_data_json: &serde_json::Value,
    chain_id: u64,
) -> Result<(String, String, String, String), WalletError> {
    use alloy::dyn_abi::TypedData;

    if private_key_bytes.len() != 32 {
        return Err(WalletError::InvalidTransaction("key must be 32 bytes".to_string()));
    }
    let key = B256::from_slice(private_key_bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    let mut data = typed_data_json.clone();

    let domain = data
        .get_mut("domain")
        .ok_or_else(|| WalletError::InvalidTypedData("missing domain".to_string()))?;

    match domain.get("chainId") {
        Some(existing) => {
            // Accept both JSON number (1) and decimal string ("1")
            let existing_id = existing
                .as_u64()
                .or_else(|| existing.as_str().and_then(|s| s.parse::<u64>().ok()))
                .ok_or_else(|| {
                    WalletError::InvalidTypedData(
                        "chainId must be a number or decimal string".to_string(),
                    )
                })?;
            if existing_id != chain_id {
                return Err(WalletError::ChainIdMismatch {
                    expected: chain_id,
                    actual: existing_id,
                });
            }
            // Normalize to JSON number so alloy's U256 deserializer accepts it
            domain
                .as_object_mut()
                .ok_or_else(|| {
                    WalletError::InvalidTypedData("domain must be an object".to_string())
                })?
                .insert("chainId".to_string(), serde_json::json!(existing_id));
        }
        None => {
            domain
                .as_object_mut()
                .ok_or_else(|| WalletError::InvalidTypedData("domain must be an object".to_string()))?
                .insert("chainId".to_string(), serde_json::json!(chain_id));
        }
    }

    if let Some(types) = data.get_mut("types") {
        if let Some(obj) = types.as_object_mut() {
            obj.remove("EIP712Domain");
        }
    }

    // 校验 message 字段不超出 primaryType 的定义
    let primary_type = data
        .get("primaryType")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::InvalidTypedData("missing primaryType".to_string()))?
        .to_string();
    let defined_fields: std::collections::HashSet<String> = data
        .get("types")
        .and_then(|t| t.get(&primary_type))
        .and_then(|fields| fields.as_array())
        .ok_or_else(|| WalletError::InvalidTypedData(format!("type {} not found", primary_type)))?
        .iter()
        .filter_map(|f| f.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
        .collect();
    if let Some(msg) = data.get("message").and_then(|m| m.as_object()) {
        for key in msg.keys() {
            if !defined_fields.contains(key) {
                return Err(WalletError::InvalidTypedData(format!(
                    "message field '{}' not defined in type {}",
                    key, primary_type
                )));
            }
        }
    }

    let typed_data: TypedData = serde_json::from_value(data)
        .map_err(|e| WalletError::InvalidTypedData(e.to_string()))?;

    let hash = typed_data
        .eip712_signing_hash()
        .map_err(|e| WalletError::InvalidTypedData(e.to_string()))?;

    let sig = signer
        .sign_hash_sync(&hash)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    let sig_bytes = sig.as_bytes();
    let signature = format!("0x{}", hex::encode(&sig_bytes));
    let r = format!("0x{}", hex::encode(&sig_bytes[..32]));
    let s = format!("0x{}", hex::encode(&sig_bytes[32..64]));
    let v = format!("0x{:02x}", sig_bytes[64]);

    Ok((signature, r, s, v))
}

#[derive(Debug)]
pub struct EvmMessageSignature {
    pub message_hash: String,
    pub signature: String,
    pub r: String,
    pub s: String,
    pub v: String,
}

pub fn sign_message(private_key_bytes: &[u8], msg_bytes: &[u8]) -> Result<EvmMessageSignature, WalletError> {
    if private_key_bytes.len() != 32 {
        return Err(WalletError::InvalidTransaction("key must be 32 bytes".to_string()));
    }
    let key = B256::from_slice(private_key_bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    let hash = eip191_hash_message(msg_bytes);

    let sig = signer
        .sign_hash_sync(&hash)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    let sig_bytes = sig.as_bytes();
    let message_hash = format!("0x{}", hex::encode(hash.as_slice()));
    let signature = format!("0x{}", hex::encode(&sig_bytes));
    let r = format!("0x{}", hex::encode(&sig_bytes[..32]));
    let s = format!("0x{}", hex::encode(&sig_bytes[32..64]));
    let v = format!("0x{:02x}", sig_bytes[64]);

    Ok(EvmMessageSignature { message_hash, signature, r, s, v })
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
    use alloy::consensus::TxEnvelope;
    use alloy::eips::eip2718::{Decodable2718, Encodable2718};
    use alloy::primitives::{Address, Bytes, Signature as AlloySignature, U256};

    fn make_unsigned_type2_bytes(chain_id: u64) -> Vec<u8> {
        use alloy::consensus::{SignableTransaction, TxEip1559};
        use alloy::primitives::TxKind;
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
        out
    }

    fn recover_from_signed_type2(signed_bytes: &[u8]) -> String {
        let mut buf = signed_bytes;
        let env = TxEnvelope::decode_2718(&mut buf).unwrap();
        if let TxEnvelope::Eip1559(signed) = env {
            let recovered = signed.recover_signer().unwrap();
            format!("{:?}", recovered)
        } else {
            panic!("expected Eip1559");
        }
    }

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

    #[test]
    fn sign_evm_type2_success_sender_recoverable() {
        let kp = generate_keypair();
        let tx_bytes = make_unsigned_type2_bytes(1);

        let (signed_bytes, from, to, value) =
            sign_transaction(&kp.private_key_bytes, &tx_bytes, 1).unwrap();

        assert_eq!(from.to_lowercase(), kp.address.to_lowercase());
        assert!(to.is_some());
        assert_ne!(value, "0");

        let recovered = recover_from_signed_type2(&signed_bytes);
        assert_eq!(recovered.to_lowercase(), kp.address.to_lowercase());
    }

    #[test]
    fn sign_evm_already_signed_returns_error() {
        let kp = generate_keypair();
        let tx_bytes = make_unsigned_type2_bytes(1);

        let (signed_bytes, _, _, _) =
            sign_transaction(&kp.private_key_bytes, &tx_bytes, 1).unwrap();
        let result = sign_transaction(&kp.private_key_bytes, &signed_bytes, 1);
        assert!(matches!(result, Err(WalletError::AlreadySigned)));
    }

    #[test]
    fn sign_evm_chain_id_mismatch_returns_error() {
        let kp = generate_keypair();
        let tx_bytes = make_unsigned_type2_bytes(1);
        let result = sign_transaction(&kp.private_key_bytes, &tx_bytes, 56);
        assert!(matches!(
            result,
            Err(WalletError::ChainIdMismatch { expected: 56, actual: 1 })
        ));
    }

    #[test]
    fn sign_evm_type0_no_chain_id_injects_chain_id() {
        use alloy::consensus::{SignableTransaction, TxLegacy};
        use alloy::primitives::TxKind;
        let tx = TxLegacy {
            chain_id: None,
            nonce: 0,
            gas_price: 20_000_000_000,
            gas_limit: 21000,
            to: TxKind::Call(Address::repeat_byte(0xab)),
            value: U256::from(1000u64),
            input: Bytes::default(),
        };
        let zero_sig = AlloySignature::new(U256::ZERO, U256::ZERO, false);
        let signed = tx.into_signed(zero_sig);
        let mut out = Vec::new();
        TxEnvelope::Legacy(signed).encode_2718(&mut out);

        let kp = generate_keypair();
        let result = sign_transaction(&kp.private_key_bytes, &out, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn sign_evm_unsupported_type3_returns_error() {
        let fake_type3 = vec![0x03u8, 0x01, 0x02, 0x03];
        let kp = generate_keypair();
        let result = sign_transaction(&kp.private_key_bytes, &fake_type3, 1);
        assert!(matches!(result, Err(WalletError::UnsupportedTxType(3))));
    }

    #[test]
    fn sign_evm_invalid_rlp_returns_error() {
        let kp = generate_keypair();
        // 0x02 = type-2 envelope prefix, followed by invalid RLP data
        let garbage = b"\x02not valid rlp data here";
        let result = sign_transaction(&kp.private_key_bytes, garbage, 1);
        assert!(matches!(result, Err(WalletError::InvalidTransaction(_))));
    }

    #[test]
    fn sign_evm_preserves_non_signature_fields() {
        let kp = generate_keypair();
        let tx_bytes = make_unsigned_type2_bytes(1);

        let (signed_bytes, _, _, _) =
            sign_transaction(&kp.private_key_bytes, &tx_bytes, 1).unwrap();

        let mut buf = tx_bytes.as_slice();
        let orig = TxEnvelope::decode_2718(&mut buf).unwrap();
        let mut buf2 = signed_bytes.as_slice();
        let signed = TxEnvelope::decode_2718(&mut buf2).unwrap();

        if let (TxEnvelope::Eip1559(o), TxEnvelope::Eip1559(s)) = (orig, signed) {
            assert_eq!(o.tx().nonce, s.tx().nonce);
            assert_eq!(o.tx().gas_limit, s.tx().gas_limit);
            assert_eq!(o.tx().value, s.tx().value);
            assert_eq!(o.tx().to, s.tx().to);
            assert_eq!(o.tx().chain_id, s.tx().chain_id);
        } else {
            panic!("expected Eip1559");
        }
    }

    // --- sign_typed_data tests ---

    fn make_typed_data_json(chain_id: u64) -> serde_json::Value {
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

    #[test]
    fn sign_typed_data_success_returns_correct_lengths() {
        let kp = generate_keypair();
        let json = make_typed_data_json(1);
        let (sig, r, s, v) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();

        assert_eq!(sig.len(), 132);
        assert!(sig.starts_with("0x"));
        assert_eq!(r.len(), 66);
        assert_eq!(s.len(), 66);
        assert!(r.starts_with("0x"));
        assert!(s.starts_with("0x"));
        assert!(v == "0x1b" || v == "0x1c");
    }

    #[test]
    fn sign_typed_data_chain_id_mismatch_returns_error() {
        let kp = generate_keypair();
        let json = make_typed_data_json(1);
        let result = sign_typed_data(&kp.private_key_bytes, &json, 56);
        assert!(matches!(
            result,
            Err(WalletError::ChainIdMismatch { expected: 56, actual: 1 })
        ));
    }

    #[test]
    fn sign_typed_data_missing_chain_id_injects_chain_id() {
        let kp = generate_keypair();
        let mut json = make_typed_data_json(1);
        json["domain"].as_object_mut().unwrap().remove("chainId");
        let result = sign_typed_data(&kp.private_key_bytes, &json, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn sign_typed_data_eip712domain_in_types_is_ignored() {
        let kp = generate_keypair();
        let mut json = make_typed_data_json(1);
        json["types"]["EIP712Domain"] = serde_json::json!([
            {"name": "name",    "type": "string"},
            {"name": "version", "type": "string"},
            {"name": "chainId", "type": "uint256"}
        ]);
        let result = sign_typed_data(&kp.private_key_bytes, &json, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn sign_typed_data_message_extra_field_returns_error() {
        let kp = generate_keypair();
        let mut json = make_typed_data_json(1);
        json["message"]["extra_field"] = serde_json::json!("unexpected");
        let result = sign_typed_data(&kp.private_key_bytes, &json, 1);
        assert!(matches!(result, Err(WalletError::InvalidTypedData(_))));
    }

    #[test]
    fn sign_typed_data_same_input_same_output() {
        let kp = generate_keypair();
        let json = make_typed_data_json(1);
        let (sig1, _, _, _) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();
        let (sig2, _, _, _) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();
        assert_eq!(sig1, sig2);
    }

    // --- sign_message tests ---

    #[test]
    fn sign_message_signature_lengths_correct() {
        let kp = generate_keypair();
        let result = sign_message(&kp.private_key_bytes, b"hello world").unwrap();
        assert_eq!(result.signature.len(), 132);
        assert_eq!(result.r.len(), 66);
        assert_eq!(result.s.len(), 66);
        assert!(result.v == "0x1b" || result.v == "0x1c");
    }

    #[test]
    fn sign_message_hash_matches_eip191() {
        let kp = generate_keypair();
        let msg = b"test message";
        let result = sign_message(&kp.private_key_bytes, msg).unwrap();
        let expected_hash = eip191_hash_message(msg);
        let expected_hex = format!("0x{}", hex::encode(expected_hash.as_slice()));
        assert_eq!(result.message_hash, expected_hex);
    }

    #[test]
    fn sign_message_signer_recoverable() {
        let kp = generate_keypair();
        let msg = b"recover me";
        let result = sign_message(&kp.private_key_bytes, msg).unwrap();

        let sig_bytes = hex::decode(result.signature.trim_start_matches("0x")).unwrap();
        assert_eq!(sig_bytes.len(), 65);
        let r = U256::from_be_slice(&sig_bytes[..32]);
        let s = U256::from_be_slice(&sig_bytes[32..64]);
        let v_byte = sig_bytes[64];
        let parity = if v_byte >= 27 { v_byte - 27 != 0 } else { v_byte != 0 };
        let sig = AlloySignature::new(r, s, parity);

        let hash_bytes = hex::decode(result.message_hash.trim_start_matches("0x")).unwrap();
        let hash = B256::from_slice(&hash_bytes);
        let recovered = sig.recover_address_from_prehash(&hash).unwrap();
        assert_eq!(recovered.to_checksum(None).to_lowercase(), kp.address.to_lowercase());
    }

    #[test]
    fn sign_message_empty_bytes_succeeds() {
        let kp = generate_keypair();
        let result = sign_message(&kp.private_key_bytes, b"");
        assert!(result.is_ok());
    }

    #[test]
    fn sign_message_invalid_key_returns_error() {
        let result = sign_message(&[0u8; 16], b"hello");
        assert!(matches!(result, Err(WalletError::InvalidTransaction(_))));
    }

    // --- chainId as decimal string ---

    #[test]
    fn sign_typed_data_chain_id_as_decimal_string_succeeds() {
        let kp = generate_keypair();
        let mut json = make_typed_data_json(1);
        // Replace numeric chainId with decimal string "1"
        json["domain"]["chainId"] = serde_json::json!("1");
        let result = sign_typed_data(&kp.private_key_bytes, &json, 1);
        assert!(result.is_ok());
        let (sig, _, _, _) = result.unwrap();
        assert_eq!(sig.len(), 132);
    }

    #[test]
    fn sign_typed_data_chain_id_string_mismatch_returns_error() {
        let kp = generate_keypair();
        let mut json = make_typed_data_json(1);
        json["domain"]["chainId"] = serde_json::json!("56");
        let result = sign_typed_data(&kp.private_key_bytes, &json, 1);
        assert!(matches!(
            result,
            Err(WalletError::ChainIdMismatch { expected: 1, actual: 56 })
        ));
    }

    // --- digest correctness: signer recoverable from EIP-712 hash ---

    fn verify_sig_recovers_to(sig_hex: &str, typed_data_json: serde_json::Value, expected_address: &str) {
        use alloy::dyn_abi::TypedData;
        use alloy::primitives::{Signature as AlloySignature, U256};

        // Compute the EIP-712 signing hash independently (same logic as sign_typed_data)
        let mut data = typed_data_json;
        if let Some(types) = data.get_mut("types") {
            if let Some(obj) = types.as_object_mut() {
                obj.remove("EIP712Domain");
            }
        }
        let typed_data: TypedData = serde_json::from_value(data).unwrap();
        let hash = typed_data.eip712_signing_hash().unwrap();

        // Decode signature bytes
        let sig_bytes = hex::decode(sig_hex.trim_start_matches("0x")).unwrap();
        assert_eq!(sig_bytes.len(), 65);
        let r = U256::from_be_slice(&sig_bytes[..32]);
        let s = U256::from_be_slice(&sig_bytes[32..64]);
        // alloy serializes v as 27 (0x1b) or 28 (0x1c); normalize to recovery id (0 or 1)
        let v_byte = sig_bytes[64];
        let parity = if v_byte >= 27 { v_byte - 27 != 0 } else { v_byte != 0 };
        let sig = AlloySignature::new(r, s, parity);

        let recovered = sig.recover_address_from_prehash(&hash).unwrap();
        assert_eq!(recovered.to_checksum(None).to_lowercase(), expected_address.to_lowercase());
    }

    #[test]
    fn sign_typed_data_digest_correctness_signer_recoverable() {
        let kp = generate_keypair();
        let json = make_typed_data_json(1);
        let (sig, _, _, _) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();
        verify_sig_recovers_to(&sig, json, &kp.address);
    }

    // --- nested struct support ---

    fn make_nested_struct_typed_data() -> serde_json::Value {
        serde_json::json!({
            "domain": {"name": "TestApp", "version": "1", "chainId": 1},
            "types": {
                "Order": [
                    {"name": "item", "type": "OrderItem"},
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
        })
    }

    #[test]
    fn sign_typed_data_nested_struct_signer_recoverable() {
        let kp = generate_keypair();
        let json = make_nested_struct_typed_data();
        let (sig, _, _, _) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();
        verify_sig_recovers_to(&sig, make_nested_struct_typed_data(), &kp.address);
    }

    // --- array type support ---

    fn make_array_type_typed_data() -> serde_json::Value {
        serde_json::json!({
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
        })
    }

    #[test]
    fn sign_typed_data_array_type_signer_recoverable() {
        let kp = generate_keypair();
        let json = make_array_type_typed_data();
        let (sig, _, _, _) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();
        verify_sig_recovers_to(&sig, make_array_type_typed_data(), &kp.address);
    }
}
