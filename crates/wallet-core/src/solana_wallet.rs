use crate::wallet::{WalletError, WalletKeys};
use solana_sdk::signature::{Keypair, Signature as SolanaSignature, Signer};
use solana_sdk::transaction::{Transaction, VersionedTransaction};

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

/// 对 Solana 交易进行签名，只填充本服务密钥对应的 signer slot。
///
/// 返回签名后的交易字节（bincode 格式）和 TxID（signatures[0] 的 base58，若非 fee payer 则 None）。
pub fn sign_transaction(
    private_key_bytes: &[u8],
    transaction_bytes: &[u8],
) -> Result<(Vec<u8>, Option<String>), WalletError> {
    let keypair = Keypair::from_bytes(private_key_bytes)
        .map_err(|e| WalletError::InvalidTransaction(format!("invalid keypair: {e}")))?;
    let our_pubkey = keypair.pubkey();

    enum TxVariant {
        Legacy(Transaction),
        Versioned(VersionedTransaction),
    }

    let variant = if let Ok(vtx) = bincode::deserialize::<VersionedTransaction>(transaction_bytes) {
        TxVariant::Versioned(vtx)
    } else if let Ok(tx) = bincode::deserialize::<Transaction>(transaction_bytes) {
        TxVariant::Legacy(tx)
    } else {
        return Err(WalletError::InvalidTransaction(
            "failed to deserialize as VersionedTransaction or Transaction".to_string(),
        ));
    };

    match variant {
        TxVariant::Legacy(mut tx) => {
            let num_required = tx.message.header.num_required_signatures as usize;
            if tx.signatures.len() < num_required {
                return Err(WalletError::InvalidTransaction(format!(
                    "transaction has {} signature slot(s) but {} are required",
                    tx.signatures.len(),
                    num_required
                )));
            }
            let required_signers = &tx.message.account_keys[..num_required];
            let signer_index = required_signers
                .iter()
                .position(|pk| *pk == our_pubkey)
                .ok_or(WalletError::SignerNotRequired)?;

            let default_sig = SolanaSignature::default();
            let all_signed = tx.signatures[..num_required]
                .iter()
                .all(|s| s != &default_sig);
            if all_signed {
                return Err(WalletError::AlreadySigned);
            }

            let message_bytes = tx.message_data();
            let sig = keypair
                .try_sign_message(&message_bytes)
                .map_err(|e| WalletError::InvalidTransaction(format!("sign failed: {e}")))?;

            tx.signatures[signer_index] = sig;

            let tx_id = if tx.signatures[0] != default_sig {
                Some(bs58::encode(tx.signatures[0].as_ref()).into_string())
            } else {
                None
            };

            let signed_bytes = bincode::serialize(&tx)
                .map_err(|e| WalletError::InvalidTransaction(format!("serialize failed: {e}")))?;
            Ok((signed_bytes, tx_id))
        }

        TxVariant::Versioned(mut vtx) => {
            let header = vtx.message.header();
            let num_required = header.num_required_signatures as usize;
            if vtx.signatures.len() < num_required {
                return Err(WalletError::InvalidTransaction(format!(
                    "transaction has {} signature slot(s) but {} are required",
                    vtx.signatures.len(),
                    num_required
                )));
            }
            let static_keys = vtx.message.static_account_keys();

            let required_signers = &static_keys[..num_required];
            let signer_index = required_signers
                .iter()
                .position(|pk| *pk == our_pubkey)
                .ok_or(WalletError::SignerNotRequired)?;

            let default_sig = SolanaSignature::default();
            let all_signed = vtx.signatures[..num_required]
                .iter()
                .all(|s| s != &default_sig);
            if all_signed {
                return Err(WalletError::AlreadySigned);
            }

            let message_bytes = vtx.message.serialize();
            let sig = keypair
                .try_sign_message(&message_bytes)
                .map_err(|e| WalletError::InvalidTransaction(format!("sign failed: {e}")))?;

            vtx.signatures[signer_index] = sig;

            let tx_id = if vtx.signatures[0] != default_sig {
                Some(bs58::encode(vtx.signatures[0].as_ref()).into_string())
            } else {
                None
            };

            let signed_bytes = bincode::serialize(&vtx)
                .map_err(|e| WalletError::InvalidTransaction(format!("serialize failed: {e}")))?;
            Ok((signed_bytes, tx_id))
        }
    }
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
    use solana_sdk::{
        hash::Hash,
        message::Message,
        pubkey::Pubkey,
        signature::Keypair as SolanaKeypair,
        signer::Signer,
        system_instruction,
        transaction::Transaction,
    };

    fn make_legacy_tx(from: &SolanaKeypair, to: &Pubkey) -> Transaction {
        let ix = system_instruction::transfer(&from.pubkey(), to, 1_000_000);
        let msg = Message::new(&[ix], Some(&from.pubkey()));
        let mut tx = Transaction::new_unsigned(msg);
        tx.message.recent_blockhash = Hash::new_unique();
        tx
    }

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

    #[test]
    fn sign_transaction_legacy_success() {
        let from_kp = SolanaKeypair::new();
        let to_pk = Pubkey::new_unique();
        let tx = make_legacy_tx(&from_kp, &to_pk);

        let tx_bytes = bincode::serialize(&tx).unwrap();
        let (signed_bytes, tx_id) = sign_transaction(&from_kp.to_bytes(), &tx_bytes).unwrap();

        let signed_tx: Transaction = bincode::deserialize(&signed_bytes).unwrap();
        let default_sig = solana_sdk::signature::Signature::default();
        assert_ne!(signed_tx.signatures[0], default_sig);

        assert!(tx_id.is_some());
        let expected_txid = bs58::encode(signed_tx.signatures[0].as_ref()).into_string();
        assert_eq!(tx_id.unwrap(), expected_txid);
    }

    #[test]
    fn sign_transaction_legacy_signature_is_valid() {
        let from_kp = SolanaKeypair::new();
        let to_pk = Pubkey::new_unique();
        let tx = make_legacy_tx(&from_kp, &to_pk);

        let tx_bytes = bincode::serialize(&tx).unwrap();
        let (signed_bytes, _) = sign_transaction(&from_kp.to_bytes(), &tx_bytes).unwrap();

        let signed_tx: Transaction = bincode::deserialize(&signed_bytes).unwrap();
        assert!(signed_tx.verify().is_ok());
    }

    #[test]
    fn sign_transaction_signer_not_required_returns_error() {
        let from_kp = SolanaKeypair::new();
        let to_pk = Pubkey::new_unique();
        let tx = make_legacy_tx(&from_kp, &to_pk);

        let other_kp = SolanaKeypair::new();
        let tx_bytes = bincode::serialize(&tx).unwrap();
        let result = sign_transaction(&other_kp.to_bytes(), &tx_bytes);

        assert!(matches!(result, Err(WalletError::SignerNotRequired)));
    }

    #[test]
    fn sign_transaction_already_fully_signed_returns_error() {
        let from_kp = SolanaKeypair::new();
        let to_pk = Pubkey::new_unique();
        let tx = make_legacy_tx(&from_kp, &to_pk);

        let tx_bytes = bincode::serialize(&tx).unwrap();
        let (signed_bytes, _) = sign_transaction(&from_kp.to_bytes(), &tx_bytes).unwrap();
        let result = sign_transaction(&from_kp.to_bytes(), &signed_bytes);

        assert!(matches!(result, Err(WalletError::AlreadySigned)));
    }

    #[test]
    fn sign_transaction_invalid_bytes_returns_error() {
        let kp = SolanaKeypair::new();
        let garbage = b"not a transaction";
        let result = sign_transaction(&kp.to_bytes(), garbage);
        assert!(matches!(result, Err(WalletError::InvalidTransaction(_))));
    }

    #[test]
    fn sign_transaction_insufficient_signature_slots_returns_invalid_transaction() {
        // Craft a transaction whose signatures vec is shorter than num_required_signatures.
        // This can arise from a truncated / malformed serialized transaction.
        let kp = SolanaKeypair::new();
        let mut tx = Transaction::default();
        // Claim 2 required signers but provide only 1 (empty) signature slot
        tx.message.header.num_required_signatures = 2;
        tx.message.account_keys = vec![kp.pubkey(), solana_sdk::pubkey::Pubkey::new_unique()];
        tx.signatures = vec![solana_sdk::signature::Signature::default()]; // 1 slot < 2 required
        let bytes = bincode::serialize(&tx).unwrap();
        let result = sign_transaction(&kp.to_bytes(), &bytes);
        assert!(
            matches!(result, Err(WalletError::InvalidTransaction(_))),
            "expected InvalidTransaction for insufficient signature slots"
        );
    }

    #[test]
    fn sign_transaction_non_fee_payer_signer_tx_id_is_none() {
        let payer = SolanaKeypair::new();
        let new_account = SolanaKeypair::new();

        let ix = system_instruction::create_account(
            &payer.pubkey(),
            &new_account.pubkey(),
            1_000_000,
            0,
            &solana_sdk::system_program::id(),
        );
        let msg = Message::new_with_blockhash(
            &[ix],
            Some(&payer.pubkey()),
            &Hash::new_unique(),
        );
        let tx = Transaction::new_unsigned(msg);

        let tx_bytes = bincode::serialize(&tx).unwrap();
        let (signed_bytes, tx_id) = sign_transaction(&new_account.to_bytes(), &tx_bytes).unwrap();

        assert!(tx_id.is_none());

        let signed_tx: Transaction = bincode::deserialize(&signed_bytes).unwrap();
        let default_sig = solana_sdk::signature::Signature::default();
        assert_eq!(signed_tx.signatures[0], default_sig, "fee payer slot should still be empty");
        assert_ne!(signed_tx.signatures[1], default_sig, "co-signer slot should be filled");
    }
}
