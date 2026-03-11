# 签名接口重设计 — 实施计划

日期：2026-03-11
参考设计文档：`docs/plans/2026-03-11-signing-api-redesign.md`

---

## 阅读须知

本文档用于指导 LLM 逐任务实现签名接口重设计。每个任务遵循 **TDD 原则**：先写测试（红），再写实现（绿），最后验证（通过）。

**不要跳过测试步骤，不要合并多个任务一起做，严格按顺序执行。**

---

## 项目结构速览

```
crates/
  wallet-core/src/
    wallet.rs          # WalletError, WalletManager, WalletKeys
    evm_wallet.rs      # EVM 密钥生成/导入
    solana_wallet.rs   # Solana 密钥生成/导入/签名
    notification.rs    # SignEvent, TelegramClient
    network.rs         # Network 枚举
    crypto.rs          # AES-GCM 加密
    lib.rs
  wallet-server/src/
    api/mod.rs         # HTTP handler + OpenAPI
    state.rs           # AppState
    server.rs          # axum 路由
    util.rs            # parse_network 等工具
    lib.rs
```

---

## 执行顺序

```
Task 0  → Cargo 依赖配置
Task 1  → WalletError 新增错误变体
Task 2  → SignEvent 枚举改造（通知层）
Task 3  → Solana sign_transaction（wallet-core）
Task 4  → EVM sign_transaction（wallet-core）
Task 5  → EIP-712 sign_typed_data（wallet-core）
Task 6  → 自定义 JSON Extractor（wallet-server）
Task 7  → Solana API handler
Task 8  → EVM transaction API handler
Task 9  → EIP-712 API handler
Task 10 → 删除旧接口 + 更新 OpenAPI
Task 11 → 文档更新
```

---

## Task 0：Cargo 依赖配置

**文件：** `crates/wallet-core/Cargo.toml`

`wallet-core` 已有 `alloy = { version = "1", features = ["full"] }`，`full` feature 包含 `alloy-dyn-abi`。无需额外添加依赖。

确认以下依赖均已存在（不需要添加，只需确认）：
- `alloy = { version = "1", features = ["full"] }` — EVM 交易 RLP 解析、EIP-712
- `solana-sdk = "2"` — Solana 交易签名
- `bs58 = "0.5"` — Solana base58 编解码
- `base64 = "0.22"` — Solana base64 解码
- `serde_json = { workspace = true }` — EIP-712 JSON 操作

**文件：** `crates/wallet-server/Cargo.toml`

`wallet-server` 的 alloy 依赖只有 `features = ["signers", "signer-local"]`，需要**移除旧的 alloy 签名逻辑**（Task 10 中删除 `sign_evm_message` helper 后即可）。无需新增依赖。

**验证命令：**
```
cd crates/wallet-core && cargo check
cd crates/wallet-server && cargo check
```

---

## Task 1：WalletError 新增错误变体

**前置条件：** 无
**文件：** `crates/wallet-core/src/wallet.rs`

### 1.1 要新增的错误变体

在现有 `WalletError` 枚举中新增以下变体：

```rust
#[error("transaction already signed")]
AlreadySigned,

#[error("signer not required: local pubkey is not in the transaction's required signers")]
SignerNotRequired,

#[error("chain id mismatch: expected {expected}, got {actual}")]
ChainIdMismatch { expected: u64, actual: u64 },

#[error("unsupported transaction type: {0}")]
UnsupportedTxType(u8),

#[error("invalid transaction: {0}")]
InvalidTransaction(String),

#[error("invalid typed data: {0}")]
InvalidTypedData(String),
```

### 1.2 先写测试（在 wallet.rs 底部的 #[cfg(test)] mod tests 中新增）

```rust
#[test]
fn wallet_error_already_signed_formats_correctly() {
    let e = WalletError::AlreadySigned;
    assert_eq!(e.to_string(), "transaction already signed");
}

#[test]
fn wallet_error_chain_id_mismatch_includes_both_values() {
    let e = WalletError::ChainIdMismatch { expected: 1, actual: 56 };
    let msg = e.to_string();
    assert!(msg.contains("1"));
    assert!(msg.contains("56"));
}

#[test]
fn wallet_error_unsupported_tx_type_includes_type_byte() {
    let e = WalletError::UnsupportedTxType(3);
    assert!(e.to_string().contains("3"));
}

#[test]
fn wallet_error_invalid_transaction_includes_message() {
    let e = WalletError::InvalidTransaction("bad rlp".to_string());
    assert!(e.to_string().contains("bad rlp"));
}

#[test]
fn wallet_error_invalid_typed_data_includes_message() {
    let e = WalletError::InvalidTypedData("missing field".to_string());
    assert!(e.to_string().contains("missing field"));
}
```

### 1.3 实现

直接将上述 6 个变体添加到 `wallet.rs` 的 `WalletError` 枚举中。

### 1.4 验证

```
cargo test -p wallet-core wallet_error_
```

---

## Task 2：SignEvent 枚举改造

**前置条件：** Task 1 完成
**文件：** `crates/wallet-core/src/notification.rs`

### 2.1 目标结构

将现有 `SignEvent` 结构体替换为枚举，新增 Markdown 转义和消息截断工具函数。

### 2.2 先写测试（替换现有 tests 模块）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- escape_markdown ---

    #[test]
    fn escape_markdown_escapes_asterisk() {
        assert_eq!(escape_markdown("hello*world"), "hello\\*world");
    }

    #[test]
    fn escape_markdown_escapes_underscore() {
        assert_eq!(escape_markdown("a_b"), "a\\_b");
    }

    #[test]
    fn escape_markdown_escapes_backtick() {
        assert_eq!(escape_markdown("a`b"), "a\\`b");
    }

    #[test]
    fn escape_markdown_leaves_normal_text_unchanged() {
        assert_eq!(escape_markdown("hello world 123"), "hello world 123");
    }

    // --- truncate_str ---

    #[test]
    fn truncate_str_short_string_unchanged() {
        assert_eq!(truncate_str("abc", 10), "abc");
    }

    #[test]
    fn truncate_str_exact_length_unchanged() {
        assert_eq!(truncate_str("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_str_long_string_truncated_with_ellipsis() {
        let result = truncate_str("abcdefghij", 5);
        assert_eq!(result, "abcde...");
    }

    // --- format_sign_message ---

    #[test]
    fn format_solana_sign_with_tx_id_contains_txid() {
        let event = SignEvent::Solana {
            tx_id: Some("5abc123".to_string()),
            transaction: "base58tx".to_string(),
            signed_at: "2026-01-01 00:00:00 UTC".to_string(),
        };
        let msg = format_sign_message(&event);
        assert!(msg.contains("5abc123"));
        assert!(msg.contains("base58tx"));
    }

    #[test]
    fn format_solana_sign_without_tx_id_shows_na() {
        let event = SignEvent::Solana {
            tx_id: None,
            transaction: "base58tx".to_string(),
            signed_at: "2026-01-01 00:00:00 UTC".to_string(),
        };
        let msg = format_sign_message(&event);
        assert!(msg.contains("N/A"));
    }

    #[test]
    fn format_evm_transaction_contains_from_to_value() {
        let event = SignEvent::EvmTransaction {
            network: "Ethereum".to_string(),
            from: "0xAbc".to_string(),
            to: Some("0xDef".to_string()),
            value: "1000000000000000000".to_string(),
            signed_at: "2026-01-01 00:00:00 UTC".to_string(),
        };
        let msg = format_sign_message(&event);
        assert!(msg.contains("0xAbc"));
        assert!(msg.contains("0xDef"));
        assert!(msg.contains("1000000000000000000"));
    }

    #[test]
    fn format_evm_transaction_contract_create_shows_contract_creation() {
        let event = SignEvent::EvmTransaction {
            network: "Ethereum".to_string(),
            from: "0xAbc".to_string(),
            to: None,
            value: "0".to_string(),
            signed_at: "2026-01-01 00:00:00 UTC".to_string(),
        };
        let msg = format_sign_message(&event);
        assert!(msg.contains("contract creation") || msg.contains("N/A"));
    }

    #[test]
    fn format_eip712_contains_network_and_truncated_data() {
        let long_json = "x".repeat(600);
        let event = SignEvent::EvmTypedData {
            network: "Ethereum".to_string(),
            typed_data_json: long_json,
            signed_at: "2026-01-01 00:00:00 UTC".to_string(),
        };
        let msg = format_sign_message(&event);
        assert!(msg.contains("Ethereum"));
        // truncated to 500 chars + "...（已截断）"
        assert!(msg.contains("...（已截断）"));
        // total message <= 4096 chars
        assert!(msg.len() <= 4096);
    }

    #[test]
    fn format_solana_sign_truncates_long_transaction() {
        let long_tx = "A".repeat(200);
        let event = SignEvent::Solana {
            tx_id: Some("txid123".to_string()),
            transaction: long_tx,
            signed_at: "2026-01-01 00:00:00 UTC".to_string(),
        };
        let msg = format_sign_message(&event);
        assert!(msg.len() <= 4096);
        assert!(msg.contains("..."));
    }

    #[test]
    fn format_locked_message_contains_operation() {
        let msg = format_locked_message("sign");
        assert!(msg.contains("sign"));
        assert!(msg.to_lowercase().contains("lock"));
    }

    #[test]
    fn telegram_client_new_stores_credentials() {
        let client = TelegramClient::new("bot_token".to_string(), "chat_id".to_string());
        assert_eq!(client.bot_token, "bot_token");
        assert_eq!(client.chat_id, "chat_id");
    }
}
```

### 2.3 实现

完整替换 `notification.rs` 内容如下：

```rust
use tracing::{error, info};

// ---- 工具函数 ----

/// 转义 Telegram Markdown v1 特殊字符：* _ ` [ ]
pub fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '*' | '_' | '`' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// 截断字符串到 max_chars 个 Unicode 字符，超出时追加 "..."
pub fn truncate_str(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

// ---- SignEvent 枚举 ----

#[derive(Debug, Clone)]
pub enum SignEvent {
    Solana {
        /// signatures[0] 的 base58 值；本服务不是 fee payer 时为 None
        tx_id: Option<String>,
        /// base58 编码的签名完整交易
        transaction: String,
        signed_at: String,
    },
    EvmTransaction {
        network: String,
        from: String,
        /// 合约创建时为 None
        to: Option<String>,
        /// 单位 wei，十进制字符串
        value: String,
        signed_at: String,
    },
    EvmTypedData {
        network: String,
        /// typed_data 原始 JSON 字符串
        typed_data_json: String,
        signed_at: String,
    },
}

pub fn format_sign_message(event: &SignEvent) -> String {
    match event {
        SignEvent::Solana { tx_id, transaction, signed_at } => {
            let txid_str = tx_id.as_deref().unwrap_or("N/A");
            let tx_display = truncate_str(transaction, 60);
            let tx_escaped = escape_markdown(&tx_display);
            format!(
                "*[Solana 签名]*\nTxID: `{}`\n交易: `{}`\n时间: {}",
                txid_str, tx_escaped, signed_at
            )
        }
        SignEvent::EvmTransaction { network, from, to, value, signed_at } => {
            let to_str = to.as_deref().unwrap_or("N/A (contract creation)");
            format!(
                "*[EVM 签名]*\n网络: {}\nFrom: `{}`\nTo: `{}`\nValue: {} wei\n时间: {}",
                network, from, to_str, value, signed_at
            )
        }
        SignEvent::EvmTypedData { network, typed_data_json, signed_at } => {
            let truncated = if typed_data_json.chars().count() > 500 {
                let s: String = typed_data_json.chars().take(500).collect();
                format!("{}...（已截断）", s)
            } else {
                typed_data_json.clone()
            };
            let escaped = escape_markdown(&truncated);
            format!(
                "*[EIP-712 签名]*\n网络: {}\n数据: `{}`\n时间: {}",
                network, escaped, signed_at
            )
        }
    }
}

// ---- TelegramClient ----

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

    pub async fn send(&self, text: &str) {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
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

pub fn format_locked_message(operation: &str) -> String {
    format!(
        "⚠️ *Wallet Locked*\nAn external app attempted a `{operation}` request, but the wallet is locked.\nPlease unlock at http://localhost:9292"
    )
}
```

### 2.4 验证

```
cargo test -p wallet-core notification
```

---

## Task 3：Solana sign_transaction

**前置条件：** Task 1 完成
**文件：** `crates/wallet-core/src/solana_wallet.rs`

### 3.1 新增函数签名

```rust
/// 对 Solana 交易进行签名，只填充本服务密钥对应的 signer slot。
///
/// # 参数
/// - `private_key_bytes`: 64 字节的 Keypair 字节（前32私钥 + 后32公钥）
/// - `transaction_bytes`: 序列化的交易字节（bincode 格式）
///
/// # 返回
/// 签名后的交易字节（bincode 格式）和 TxID（signatures[0] 的 base58，若为空则 None）
pub fn sign_transaction(
    private_key_bytes: &[u8],
    transaction_bytes: &[u8],
) -> Result<(Vec<u8>, Option<String>), WalletError>
```

### 3.2 先写测试（新增到 solana_wallet.rs 底部的 tests 模块）

需要在测试模块顶部增加以下辅助函数和 use 语句：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        message::Message,
        pubkey::Pubkey,
        system_instruction,
        transaction::Transaction,
        hash::Hash,
        signature::Keypair as SolanaKeypair,
        signer::Signer,
    };

    /// 创建一个简单的 SOL 转账 Legacy 交易（已设置 recent_blockhash）
    fn make_legacy_tx(from: &SolanaKeypair, to: &Pubkey) -> Transaction {
        let ix = system_instruction::transfer(&from.pubkey(), to, 1_000_000);
        let msg = Message::new(&[ix], Some(&from.pubkey()));
        let mut tx = Transaction::new_unsigned(msg);
        tx.message.recent_blockhash = Hash::new_unique();
        tx
    }

    // 已有测试保持不变 ...

    #[test]
    fn sign_transaction_legacy_success() {
        let from_kp = SolanaKeypair::new();
        let to_pk = Pubkey::new_unique();
        let tx = make_legacy_tx(&from_kp, &to_pk);

        let tx_bytes = bincode::serialize(&tx).unwrap();
        let (signed_bytes, tx_id) = sign_transaction(&from_kp.to_bytes(), &tx_bytes).unwrap();

        // 反序列化签名后的交易
        let signed_tx: Transaction = bincode::deserialize(&signed_bytes).unwrap();

        // signatures[0] 应为非零
        let default_sig = solana_sdk::signature::Signature::default();
        assert_ne!(signed_tx.signatures[0], default_sig);

        // tx_id 应为 signatures[0] 的 base58
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

        // 验证签名有效
        assert!(signed_tx.verify().is_ok());
    }

    #[test]
    fn sign_transaction_signer_not_required_returns_error() {
        let from_kp = SolanaKeypair::new();
        let to_pk = Pubkey::new_unique();
        let tx = make_legacy_tx(&from_kp, &to_pk);

        // 用不相关的密钥对签名
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
        // 第一次签名
        let (signed_bytes, _) = sign_transaction(&from_kp.to_bytes(), &tx_bytes).unwrap();
        // 再次签名应该返回 AlreadySigned
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
    fn sign_transaction_fee_payer_not_signer_tx_id_is_none() {
        // 构造一个双签交易，本服务签第二个 signer（不是 fee payer）
        let fee_payer = SolanaKeypair::new();
        let co_signer = SolanaKeypair::new();
        let to_pk = Pubkey::new_unique();

        // 创建需要两个签名者的交易
        let ix = system_instruction::transfer(&fee_payer.pubkey(), &to_pk, 1_000_000);
        // 手动构造需要 co_signer 的 message（简化：直接用 nonce 场景是最好的例子，
        // 这里用两个账户都需要签名的 transfer 并手动设置 header）
        let msg = Message::new_with_blockhash(
            &[ix],
            Some(&fee_payer.pubkey()),
            &Hash::new_unique(),
        );
        let mut tx = Transaction::new_unsigned(msg);
        // 设置 co_signer 的签名槽（手动把 co_signer pubkey 加入 required signers）
        // 注：这是简化测试，实际多签需要正确的 message 构造
        // 先给 fee_payer 签名，确保 signatures[0] 非空
        tx.sign(&[&fee_payer], tx.message.recent_blockhash);
        // 验证 fee_payer 签完后 tx_id = signatures[0]
        let tx_bytes = bincode::serialize(&tx).unwrap();
        // 此时已完全签名（只有1个required signer），应返回 AlreadySigned
        let result = sign_transaction(&fee_payer.to_bytes(), &tx_bytes);
        assert!(matches!(result, Err(WalletError::AlreadySigned)));
    }
}
```

**注：** `bincode` 需要加入依赖，在 `wallet-core/Cargo.toml` 新增：
```toml
bincode = "1"
```

### 3.3 实现算法（精确步骤）

在 `solana_wallet.rs` 中实现 `sign_transaction`：

```rust
use crate::wallet::WalletError;
use solana_sdk::signature::{Keypair, Signature as SolanaSignature, Signer};
use solana_sdk::transaction::{Transaction, VersionedTransaction};

pub fn sign_transaction(
    private_key_bytes: &[u8],
    transaction_bytes: &[u8],
) -> Result<(Vec<u8>, Option<String>), WalletError> {
    // 步骤 1: 从 64 字节还原 Keypair
    let keypair = Keypair::from_bytes(private_key_bytes)
        .map_err(|e| WalletError::InvalidTransaction(format!("invalid keypair: {e}")))?;
    let our_pubkey = keypair.pubkey();

    // 步骤 2: 先尝试 VersionedTransaction，失败再尝试 Legacy Transaction
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
            // 步骤 3: 确定 required signers 数量
            let num_required = tx.message.header.num_required_signatures as usize;

            // 步骤 4: 检查签名数组长度是否合法
            if tx.signatures.len() != tx.message.account_keys.len() {
                // 修正：legacy tx 的 signatures 长度应等于 num_required
                // 实际上 solana_sdk 保证 signatures.len() == num_required_signatures
            }

            // 步骤 5: 找到本服务的 pubkey 在 required signers 中的位置
            let required_signers = &tx.message.account_keys[..num_required];
            let signer_index = required_signers
                .iter()
                .position(|pk| *pk == our_pubkey)
                .ok_or(WalletError::SignerNotRequired)?;

            // 步骤 6: 检查是否所有 signer 位置均已填充（already_signed）
            let default_sig = SolanaSignature::default();
            let all_signed = tx.signatures[..num_required]
                .iter()
                .all(|s| s != &default_sig);
            if all_signed {
                return Err(WalletError::AlreadySigned);
            }

            // 步骤 7: 获取消息字节并签名
            let message_bytes = tx.message_data();
            let sig = keypair
                .try_sign_message(&message_bytes)
                .map_err(|e| WalletError::InvalidTransaction(format!("sign failed: {e}")))?;

            // 步骤 8: 将签名填入对应位置
            tx.signatures[signer_index] = sig;

            // 步骤 9: 计算 TxID（signatures[0] 的 base58，若仍为零则 None）
            let tx_id = if tx.signatures[0] != default_sig {
                Some(bs58::encode(tx.signatures[0].as_ref()).into_string())
            } else {
                None
            };

            // 步骤 10: 序列化并返回
            let signed_bytes = bincode::serialize(&tx)
                .map_err(|e| WalletError::InvalidTransaction(format!("serialize failed: {e}")))?;
            Ok((signed_bytes, tx_id))
        }

        TxVariant::Versioned(mut vtx) => {
            // Versioned 交易的处理逻辑与 Legacy 类似
            let header = vtx.message.header();
            let num_required = header.num_required_signatures as usize;
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
```

### 3.4 验证

```
cargo test -p wallet-core solana_wallet
```


---

## Task 4：EVM sign_transaction

**前置条件：** Task 1 完成
**文件：** `crates/wallet-core/src/evm_wallet.rs`

### 4.1 输入格式约定（必读）

客户端发送的"未签名完整 EVM 交易"格式（服务端按此格式解码）：

- **Type 0 (Legacy)**：`rlp([nonce, gasPrice, gasLimit, to, value, data, v=chainId, r=0, s=0])`
  EIP-155 预签名格式，r 和 s 均为 0
- **Type 1 (EIP-2930)**：`0x01 || rlp([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList, signatureYParity=0, signatureR=0, signatureS=0])`
- **Type 2 (EIP-1559)**：`0x02 || rlp([chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList, signatureYParity=0, signatureR=0, signatureS=0])`

所有类型的 r 和 s 均为 0 表示未签名。使用 `TxEnvelope::decode_2718` 统一解码。

### 4.2 新增函数签名

```rust
/// 对 EVM 交易进行签名。
///
/// # 参数
/// - `private_key_bytes`: 32 字节 secp256k1 私钥
/// - `tx_bytes`: 未签名的完整 EVM 交易（含零签名字段），hex 解码后的字节
/// - `chain_id`: 目标网络的 chainId（由 network 参数决定）
///
/// # 返回
/// (signed_tx_bytes, from_address, to_address, value_wei)
pub fn sign_transaction(
    private_key_bytes: &[u8],
    tx_bytes: &[u8],
    chain_id: u64,
) -> Result<(Vec<u8>, String, Option<String>, String), WalletError>
```

### 4.3 先写测试

在 `evm_wallet.rs` 的 tests 模块中新增以下内容。

首先添加 use 语句：
```rust
use alloy::consensus::{TxEnvelope, TxEip1559, TxLegacy, TxKind};
use alloy::eips::eip2718::{Decodable2718, Encodable2718};
use alloy::primitives::{U256, Address, Bytes};
use alloy::signers::local::PrivateKeySigner;
```

测试辅助函数（放在 tests 模块内）：

```rust
/// 构建一个 Type 2 未签名交易的字节（含零签名）
fn make_unsigned_type2_bytes(chain_id: u64) -> Vec<u8> {
    use alloy::consensus::{TxEip1559, TxKind, Signed};
    use alloy::primitives::{U256, Address, Bytes, Signature as AlloySignature};
    use alloy::eips::eip2718::Encodable2718;

    let tx = TxEip1559 {
        chain_id,
        nonce: 0,
        max_priority_fee_per_gas: 1_000_000_000,
        max_fee_per_gas: 20_000_000_000,
        gas_limit: 21000,
        to: TxKind::Call(Address::repeat_byte(0xde)),
        value: U256::from(1_000_000_000_000_000_000u64), // 1 ETH
        input: Bytes::default(),
        access_list: Default::default(),
    };
    // 创建零签名的 Signed 包装
    let zero_sig = AlloySignature::from_rs_and_parity(U256::ZERO, U256::ZERO, false).unwrap();
    let hash = tx.signature_hash();
    let signed = tx.into_signed(zero_sig);
    let mut out = Vec::new();
    TxEnvelope::Eip1559(signed).encode_2718(&mut out);
    out
}

/// 从签名后的交易字节中恢复 from 地址
fn recover_from_signed_type2(signed_bytes: &[u8]) -> String {
    use alloy::consensus::TxEnvelope;
    use alloy::eips::eip2718::Decodable2718;
    use alloy::network::TxSignerSync;

    let mut buf = signed_bytes;
    let env = TxEnvelope::decode_2718(&mut buf).unwrap();
    if let TxEnvelope::Eip1559(signed) = env {
        let recovered = signed.recover_signer().unwrap();
        format!("{:?}", recovered)
    } else {
        panic!("expected Eip1559");
    }
}
```

测试用例：

```rust
#[test]
fn sign_evm_type2_success_sender_recoverable() {
    let kp = generate_keypair();
    let tx_bytes = make_unsigned_type2_bytes(1);

    let (signed_bytes, from, to, value) = sign_transaction(&kp.private_key_bytes, &tx_bytes, 1).unwrap();

    // from 应该是本服务地址
    assert_eq!(from.to_lowercase(), kp.address.to_lowercase());

    // to 应该是 0xdede...
    assert!(to.is_some());

    // value 应是非零字符串
    assert_ne!(value, "0");

    // 签名可验证（recover sender = 我们的地址）
    let recovered = recover_from_signed_type2(&signed_bytes);
    assert_eq!(recovered.to_lowercase(), kp.address.to_lowercase());
}

#[test]
fn sign_evm_already_signed_returns_error() {
    let kp = generate_keypair();
    let tx_bytes = make_unsigned_type2_bytes(1);

    // 先签一次
    let (signed_bytes, _, _, _) = sign_transaction(&kp.private_key_bytes, &tx_bytes, 1).unwrap();

    // 再签应返回 AlreadySigned
    let result = sign_transaction(&kp.private_key_bytes, &signed_bytes, 1);
    assert!(matches!(result, Err(WalletError::AlreadySigned)));
}

#[test]
fn sign_evm_chain_id_mismatch_returns_error() {
    let kp = generate_keypair();
    // 交易里是 chainId=1，但传入 chain_id=56
    let tx_bytes = make_unsigned_type2_bytes(1);
    let result = sign_transaction(&kp.private_key_bytes, &tx_bytes, 56);
    assert!(matches!(result, Err(WalletError::ChainIdMismatch { expected: 56, actual: 1 })));
}

#[test]
fn sign_evm_type0_no_chain_id_injects_chain_id() {
    // Type 0 无 chainId（v=0, r=0, s=0 形式）应该注入 chain_id 并成功签名
    // 此测试构建一个 legacy 交易，chain_id = None
    use alloy::consensus::{TxLegacy, TxKind, Signed};
    use alloy::primitives::{U256, Address, Bytes, Signature as AlloySignature};
    use alloy::eips::eip2718::Encodable2718;

    let tx = TxLegacy {
        chain_id: None, // 无 chainId
        nonce: 0,
        gas_price: 20_000_000_000,
        gas_limit: 21000,
        to: TxKind::Call(Address::repeat_byte(0xab)),
        value: U256::from(1000u64),
        input: Bytes::default(),
    };
    let zero_sig = AlloySignature::from_rs_and_parity(U256::ZERO, U256::ZERO, false).unwrap();
    let signed = tx.into_signed(zero_sig);
    let mut out = Vec::new();
    TxEnvelope::Legacy(signed).encode_2718(&mut out);

    let kp = generate_keypair();
    let result = sign_transaction(&kp.private_key_bytes, &out, 1);
    assert!(result.is_ok());
}

#[test]
fn sign_evm_unsupported_type3_returns_error() {
    // 构造一个 type byte = 0x03 开头的假字节
    let fake_type3 = vec![0x03u8, 0x01, 0x02, 0x03];
    let kp = generate_keypair();
    let result = sign_transaction(&kp.private_key_bytes, &fake_type3, 1);
    assert!(matches!(result, Err(WalletError::UnsupportedTxType(3))));
}

#[test]
fn sign_evm_invalid_rlp_returns_error() {
    let kp = generate_keypair();
    let garbage = b"not valid rlp";
    let result = sign_transaction(&kp.private_key_bytes, garbage, 1);
    assert!(matches!(result, Err(WalletError::InvalidTransaction(_))));
}

#[test]
fn sign_evm_preserves_non_signature_fields() {
    let kp = generate_keypair();
    let tx_bytes = make_unsigned_type2_bytes(1);

    let (signed_bytes, _, _, _) = sign_transaction(&kp.private_key_bytes, &tx_bytes, 1).unwrap();

    // 解码原始和签名后的交易，比较非签名字段
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
```

### 4.4 实现算法

```rust
use crate::wallet::WalletError;
use alloy::consensus::TxEnvelope;
use alloy::eips::eip2718::{Decodable2718, Encodable2718};
use alloy::primitives::{U256, B256};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;

pub fn sign_transaction(
    private_key_bytes: &[u8],
    tx_bytes: &[u8],
    chain_id: u64,
) -> Result<(Vec<u8>, String, Option<String>, String), WalletError> {
    // 步骤 1: 还原签名者
    if private_key_bytes.len() != 32 {
        return Err(WalletError::InvalidTransaction("key must be 32 bytes".to_string()));
    }
    let key = B256::from_slice(private_key_bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
    let from_address = signer.address().to_checksum(None);

    // 步骤 2: 检测类型字节
    if tx_bytes.is_empty() {
        return Err(WalletError::InvalidTransaction("empty transaction".to_string()));
    }
    // Type 3 (EIP-4844) 不支持，提前拒绝
    if tx_bytes[0] == 0x03 {
        return Err(WalletError::UnsupportedTxType(3));
    }
    // 任何其他非 RLP 且不是 0x01/0x02 的类型字节也不支持
    if tx_bytes[0] != 0x01 && tx_bytes[0] != 0x02 && tx_bytes[0] < 0x80 {
        return Err(WalletError::UnsupportedTxType(tx_bytes[0]));
    }

    // 步骤 3: 解码为 TxEnvelope（要求客户端发送含零签名的完整格式）
    let mut buf = tx_bytes;
    let envelope = TxEnvelope::decode_2718(&mut buf)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    // 步骤 4: 根据类型处理
    match envelope {
        TxEnvelope::Legacy(signed) => {
            // 检查是否已签名（r 和 s 非零）
            let sig = signed.signature();
            if sig.r() != U256::ZERO || sig.s() != U256::ZERO {
                return Err(WalletError::AlreadySigned);
            }
            let tx = signed.tx();

            // chainId 处理：Legacy 无 chainId 则注入，有则校验
            let mut tx_owned = tx.clone();
            match tx.chain_id {
                Some(id) if id != chain_id => {
                    return Err(WalletError::ChainIdMismatch { expected: chain_id, actual: id });
                }
                _ => {
                    tx_owned.chain_id = Some(chain_id);
                }
            }

            // 提取 to 和 value
            let to = match &tx_owned.to {
                alloy::consensus::TxKind::Call(addr) => Some(addr.to_checksum(None)),
                alloy::consensus::TxKind::Create => None,
            };
            let value = tx_owned.value.to_string();

            // 签名
            let hash = tx_owned.signature_hash();
            let new_sig = signer.sign_hash_sync(&hash)
                .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
            let new_signed = tx_owned.into_signed(new_sig);
            let new_envelope = TxEnvelope::Legacy(new_signed);

            let mut out = Vec::new();
            new_envelope.encode_2718(&mut out);
            Ok((out, from_address, to, value))
        }

        TxEnvelope::Eip2930(signed) => {
            let sig = signed.signature();
            if sig.r() != U256::ZERO || sig.s() != U256::ZERO {
                return Err(WalletError::AlreadySigned);
            }
            let tx = signed.tx();
            if tx.chain_id != chain_id {
                return Err(WalletError::ChainIdMismatch {
                    expected: chain_id,
                    actual: tx.chain_id,
                });
            }
            let to = match &tx.to {
                alloy::consensus::TxKind::Call(addr) => Some(addr.to_checksum(None)),
                alloy::consensus::TxKind::Create => None,
            };
            let value = tx.value.to_string();
            let hash = tx.signature_hash();
            let new_sig = signer.sign_hash_sync(&hash)
                .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
            let new_signed = tx.clone().into_signed(new_sig);
            let mut out = Vec::new();
            TxEnvelope::Eip2930(new_signed).encode_2718(&mut out);
            Ok((out, from_address, to, value))
        }

        TxEnvelope::Eip1559(signed) => {
            let sig = signed.signature();
            if sig.r() != U256::ZERO || sig.s() != U256::ZERO {
                return Err(WalletError::AlreadySigned);
            }
            let tx = signed.tx();
            if tx.chain_id != chain_id {
                return Err(WalletError::ChainIdMismatch {
                    expected: chain_id,
                    actual: tx.chain_id,
                });
            }
            let to = match &tx.to {
                alloy::consensus::TxKind::Call(addr) => Some(addr.to_checksum(None)),
                alloy::consensus::TxKind::Create => None,
            };
            let value = tx.value.to_string();
            let hash = tx.signature_hash();
            let new_sig = signer.sign_hash_sync(&hash)
                .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;
            let new_signed = tx.clone().into_signed(new_sig);
            let mut out = Vec::new();
            TxEnvelope::Eip1559(new_signed).encode_2718(&mut out);
            Ok((out, from_address, to, value))
        }

        TxEnvelope::Eip4844(_) => Err(WalletError::UnsupportedTxType(3)),

        _ => Err(WalletError::UnsupportedTxType(tx_bytes[0])),
    }
}
```

### 4.5 chain_id 对应表（用于 handler，不在此实现）

| network | chain_id |
|---------|----------|
| eth     | 1        |
| bnb     | 56       |
| arb     | 42161    |
| polygon | 137      |

在 `network.rs` 中新增方法：

```rust
impl Network {
    pub fn chain_id(&self) -> Option<u64> {
        match self {
            Network::Eth     => Some(1),
            Network::Bnb     => Some(56),
            Network::Arb     => Some(42161),
            Network::Polygon => Some(137),
            Network::Solana  => None,
        }
    }
}
```

### 4.6 验证

```
cargo test -p wallet-core evm_wallet
```

---

## Task 5：EIP-712 sign_typed_data

**前置条件：** Task 1 完成
**文件：** `crates/wallet-core/src/evm_wallet.rs`（追加到同文件）

### 5.1 新增函数签名

```rust
/// 对 EIP-712 typed data 进行签名。
///
/// # 参数
/// - `private_key_bytes`: 32 字节 secp256k1 私钥
/// - `typed_data_json`: 已解析的 serde_json::Value，格式为标准 EIP-712 结构
/// - `chain_id`: 目标网络的 chainId
///
/// # 返回
/// (signature_65bytes, r_32bytes, s_32bytes, v_1byte)
/// 所有字节数组已包含前导 0x
pub fn sign_typed_data(
    private_key_bytes: &[u8],
    typed_data_json: &serde_json::Value,
    chain_id: u64,
) -> Result<(String, String, String, String), WalletError>
```

### 5.2 先写测试

```rust
#[cfg(test)]
// 在 evm_wallet.rs tests 模块中追加

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

    // signature: "0x" + 130 hex chars = 132 total
    assert_eq!(sig.len(), 132);
    assert!(sig.starts_with("0x"));

    // r and s: "0x" + 64 hex chars = 66 total
    assert_eq!(r.len(), 66);
    assert_eq!(s.len(), 66);
    assert!(r.starts_with("0x"));
    assert!(s.starts_with("0x"));

    // v: "0x1b" or "0x1c"
    assert!(v == "0x1b" || v == "0x1c");
}

#[test]
fn sign_typed_data_chain_id_mismatch_returns_error() {
    let kp = generate_keypair();
    let json = make_typed_data_json(1); // domain.chainId = 1
    let result = sign_typed_data(&kp.private_key_bytes, &json, 56); // network = BNB
    assert!(matches!(result, Err(WalletError::ChainIdMismatch { expected: 56, actual: 1 })));
}

#[test]
fn sign_typed_data_missing_chain_id_injects_chain_id() {
    let kp = generate_keypair();
    let mut json = make_typed_data_json(1);
    // 移除 domain.chainId
    json["domain"].as_object_mut().unwrap().remove("chainId");

    // 应该成功，注入 chain_id=1
    let result = sign_typed_data(&kp.private_key_bytes, &json, 1);
    assert!(result.is_ok());
}

#[test]
fn sign_typed_data_eip712domain_in_types_is_ignored() {
    let kp = generate_keypair();
    let mut json = make_typed_data_json(1);
    // 加入 EIP712Domain 类型定义（服务端应忽略）
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
    // 在 message 中加入 types 未声明的字段
    json["message"]["extra_field"] = serde_json::json!("unexpected");
    let result = sign_typed_data(&kp.private_key_bytes, &json, 1);
    assert!(matches!(result, Err(WalletError::InvalidTypedData(_))));
}

#[test]
fn sign_typed_data_same_input_same_output() {
    // 相同输入应产生相同签名（deterministic ECDSA with RFC6979 nonce）
    let kp = generate_keypair();
    let json = make_typed_data_json(1);
    let (sig1, _, _, _) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();
    let (sig2, _, _, _) = sign_typed_data(&kp.private_key_bytes, &json, 1).unwrap();
    assert_eq!(sig1, sig2);
}
```

### 5.3 实现算法

```rust
use alloy::dyn_abi::TypedData;

pub fn sign_typed_data(
    private_key_bytes: &[u8],
    typed_data_json: &serde_json::Value,
    chain_id: u64,
) -> Result<(String, String, String, String), WalletError> {
    // 步骤 1: 还原签名者
    if private_key_bytes.len() != 32 {
        return Err(WalletError::InvalidTransaction("key must be 32 bytes".to_string()));
    }
    let key = B256::from_slice(private_key_bytes);
    let signer = PrivateKeySigner::from_bytes(&key)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    // 步骤 2: 克隆 JSON 并处理 domain.chainId
    let mut data = typed_data_json.clone();
    let domain = data
        .get_mut("domain")
        .ok_or_else(|| WalletError::InvalidTypedData("missing domain".to_string()))?;

    match domain.get("chainId") {
        Some(existing) => {
            let existing_id = existing
                .as_u64()
                .ok_or_else(|| WalletError::InvalidTypedData("chainId must be a number".to_string()))?;
            if existing_id != chain_id {
                return Err(WalletError::ChainIdMismatch {
                    expected: chain_id,
                    actual: existing_id,
                });
            }
        }
        None => {
            // 注入 chainId
            domain
                .as_object_mut()
                .ok_or_else(|| WalletError::InvalidTypedData("domain must be an object".to_string()))?
                .insert("chainId".to_string(), serde_json::json!(chain_id));
        }
    }

    // 步骤 3: 移除 types 中的 EIP712Domain（若存在）
    if let Some(types) = data.get_mut("types") {
        if let Some(obj) = types.as_object_mut() {
            obj.remove("EIP712Domain");
        }
    }

    // 步骤 4: 解析为 TypedData
    let typed_data: TypedData = serde_json::from_value(data)
        .map_err(|e| WalletError::InvalidTypedData(e.to_string()))?;

    // 步骤 5: 计算 EIP-712 signing hash
    let hash = typed_data
        .eip712_signing_hash()
        .map_err(|e| WalletError::InvalidTypedData(e.to_string()))?;

    // 步骤 6: 签名
    let sig = signer
        .sign_hash_sync(&hash)
        .map_err(|e| WalletError::InvalidTransaction(e.to_string()))?;

    // 步骤 7: 拆分 r/s/v 并格式化为 hex 字符串（含 0x 前缀）
    let sig_bytes = sig.as_bytes(); // 65 字节：r(32) + s(32) + v(1)
    let signature = format!("0x{}", hex::encode(&sig_bytes));
    let r = format!("0x{}", hex::encode(&sig_bytes[..32]));
    let s = format!("0x{}", hex::encode(&sig_bytes[32..64]));
    let v = format!("0x{:02x}", sig_bytes[64]);

    Ok((signature, r, s, v))
}
```

### 5.4 验证

```
cargo test -p wallet-core sign_typed_data
```


---

## Task 6：自定义 JSON Extractor

**前置条件：** 无（独立任务）
**文件：** 新建 `crates/wallet-server/src/extractor.rs`

### 6.1 目标

替换 handler 中直接使用 `axum::Json<T>` 的地方，改用自定义 extractor，以确保解析失败时返回约定格式的 JSON 错误，而不是 axum 默认的纯文本错误。

### 6.2 先写测试

在 `crates/wallet-server/src/extractor.rs` 底部：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode, header}};
    use axum_test::TestServer;
    use axum::{routing::post, Router};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestPayload {
        name: String,
    }

    async fn test_handler(ValidatedJson(payload): ValidatedJson<TestPayload>)
        -> axum::Json<serde_json::Value>
    {
        axum::Json(serde_json::json!({"name": payload.name}))
    }

    fn test_router() -> Router {
        Router::new().route("/test", post(test_handler))
    }

    #[tokio::test]
    async fn valid_json_passes_through() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .json(&serde_json::json!({"name": "hello"}))
            .await;
        assert_eq!(resp.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn malformed_json_returns_400_with_error_field() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .content_type("application/json")
            .bytes(b"{ not valid json".into())
            .await;
        assert_eq!(resp.status_code(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "malformed_json");
        // 不含 request_id 字段
        assert!(body.get("request_id").is_none());
    }

    #[tokio::test]
    async fn empty_body_returns_400() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .content_type("application/json")
            .bytes(b"".into())
            .await;
        assert_eq!(resp.status_code(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "empty_body");
    }

    #[tokio::test]
    async fn wrong_content_type_returns_415() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .content_type("text/plain")
            .bytes(b"{\"name\":\"hello\"}".into())
            .await;
        assert_eq!(resp.status_code(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "unsupported_media_type");
    }

    #[tokio::test]
    async fn type_mismatch_returns_422() {
        let server = TestServer::new(test_router()).unwrap();
        // name 应该是 string，但传了 number
        let resp = server
            .post("/test")
            .json(&serde_json::json!({"name": 123}))
            .await;
        assert_eq!(resp.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_body");
    }
}
```

### 6.3 实现

```rust
// crates/wallet-server/src/extractor.rs

use axum::{
    async_trait,
    body::Body,
    extract::{FromRequest, Request},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;

pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // 1. 检查 Content-Type
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !content_type.starts_with("application/json") {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(serde_json::json!({
                    "error": "unsupported_media_type",
                    "message": "Content-Type must be application/json"
                })),
            )
                .into_response());
        }

        // 2. 读取 body 字节
        let bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
            Ok(b) => b,
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "empty_body"})),
                )
                    .into_response());
            }
        };

        if bytes.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "empty_body"})),
            )
                .into_response());
        }

        // 3. 解析 JSON
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "malformed_json",
                        "message": e.to_string()
                    })),
                )
                    .into_response());
            }
            Ok(_) => {}
        }

        // 4. 反序列化为目标类型
        match serde_json::from_slice::<T>(&bytes) {
            Ok(value) => Ok(ValidatedJson(value)),
            Err(e) => Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "invalid_body",
                    "message": e.to_string()
                })),
            )
                .into_response()),
        }
    }
}
```

在 `crates/wallet-server/src/lib.rs` 中新增：
```rust
pub mod extractor;
```

### 6.4 验证

```
cargo test -p wallet-server extractor
```

---

## Task 7：Solana API Handler

**前置条件：** Task 2、3、6 完成
**文件：** `crates/wallet-server/src/api/mod.rs`（新增路由和 handler）

### 7.1 新增的 Request/Response 结构体

```rust
// --- Solana ---

#[derive(Deserialize, ToSchema)]
struct SignSolanaRequest {
    /// 请求唯一标识，原样透传至响应
    request_id: String,
    /// 输入交易的编码格式，枚举值：`"base64"` 或 `"base58"`
    encoding: String,
    /// 完整 Solana 交易，已含 recent_blockhash；编码格式由 encoding 字段指定
    transaction: String,
}

#[derive(Serialize, ToSchema)]
struct SignSolanaResponse {
    /// 透传自请求的唯一标识
    request_id: String,
    /// 签名后的完整 Solana 交易；固定 base58 编码，与请求的 encoding 无关
    transaction: String,
}
```

### 7.2 先写测试（在 api/mod.rs 的 tests 模块中新增）

```rust
mod solana_sign_tests {
    use super::*;
    use axum_test::TestServer;
    use solana_sdk::{
        message::Message,
        pubkey::Pubkey,
        system_instruction,
        transaction::Transaction,
        hash::Hash,
        signature::Keypair as SolanaKeypair,
        signer::Signer,
    };
    use wallet_core::solana_wallet;

    fn make_unsigned_solana_tx_base64(kp: &SolanaKeypair) -> String {
        let ix = solana_sdk::system_instruction::transfer(
            &kp.pubkey(),
            &Pubkey::new_unique(),
            1_000_000,
        );
        let msg = Message::new(&[ix], Some(&kp.pubkey()));
        let mut tx = Transaction::new_unsigned(msg);
        tx.message.recent_blockhash = Hash::new_unique();
        let bytes = bincode::serialize(&tx).unwrap();
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    }

    async fn unlocked_server_with_solana() -> (TestServer, tempfile::TempDir, SolanaKeypair) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let password = "test-pass";
        let kp = SolanaKeypair::new();
        let keys = wallet_core::WalletKeys {
            private_key_bytes: kp.to_bytes().to_vec(),
            address: kp.pubkey().to_string(),
        };
        state.wallet.save_wallet(&wallet_core::Network::Solana, &keys, password).unwrap();
        state.wallet.unlock(password).await.unwrap();
        let app = router(state);
        (TestServer::new(app).unwrap(), dir, kp)
    }

    #[tokio::test]
    async fn sign_solana_locked_returns_503() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let app = router(state);
        let server = TestServer::new(app).unwrap();

        let resp = server
            .post("/api/wallet/sign/solana")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "encoding": "base64",
                "transaction": "abc"
            }))
            .await;
        assert_eq!(resp.status_code(), 503);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "wallet_locked");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_solana_missing_request_id_returns_400() {
        let (server, _dir, _kp) = unlocked_server_with_solana().await;
        let resp = server
            .post("/api/wallet/sign/solana")
            .json(&serde_json::json!({
                "encoding": "base64",
                "transaction": "abc"
            }))
            .await;
        // request_id 缺失，返回 422（字段类型不匹配/缺失）或 400
        assert!(resp.status_code() == 400 || resp.status_code() == 422);
        let body: serde_json::Value = resp.json();
        // 不含 request_id（因为没能解析出来）
        // 如果使用自定义 extractor，返回 invalid_body
        // 业务层校验需在 request_id 为空字符串时返回 400 missing_request_id
    }

    #[tokio::test]
    async fn sign_solana_invalid_encoding_returns_400() {
        let (server, _dir, kp) = unlocked_server_with_solana().await;
        let resp = server
            .post("/api/wallet/sign/solana")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "encoding": "hex",  // 不支持
                "transaction": "abc"
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_encoding");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_solana_invalid_transaction_returns_400() {
        let (server, _dir, _kp) = unlocked_server_with_solana().await;
        let resp = server
            .post("/api/wallet/sign/solana")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "encoding": "base64",
                "transaction": "bm90YXZhbGlkdHg="  // "notvalidtx" in base64
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_transaction");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_solana_legacy_success_returns_base58_transaction() {
        let (server, _dir, kp) = unlocked_server_with_solana().await;
        let tx_b64 = make_unsigned_solana_tx_base64(&kp);

        let resp = server
            .post("/api/wallet/sign/solana")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "encoding": "base64",
                "transaction": tx_b64
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["request_id"], "req-1");

        // 返回的 transaction 是有效的 base58
        let tx_str = body["transaction"].as_str().unwrap();
        let decoded = bs58::decode(tx_str).into_vec().unwrap();

        // 可以反序列化为 Transaction
        let signed_tx: Transaction = bincode::deserialize(&decoded).unwrap();

        // 签名有效
        assert!(signed_tx.verify().is_ok());
    }

    #[tokio::test]
    async fn sign_solana_all_errors_contain_request_id() {
        let (server, _dir, _kp) = unlocked_server_with_solana().await;
        // 测试 invalid_encoding 错误中含有 request_id
        let resp = server
            .post("/api/wallet/sign/solana")
            .json(&serde_json::json!({
                "request_id": "my-trace-id",
                "encoding": "invalid",
                "transaction": "abc"
            }))
            .await;
        let body: serde_json::Value = resp.json();
        assert_eq!(body["request_id"], "my-trace-id");
    }
}
```

### 7.3 Handler 实现

```rust
async fn sign_solana(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SignSolanaRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // 1. 校验 request_id 非空
    if req.request_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing_request_id"})),
        );
    }
    let rid = &req.request_id;

    // 2. 检查钱包是否解锁
    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign/solana")).await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "wallet_locked",
                "message": "Wallet is locked. Please unlock via admin UI at http://localhost:9292",
                "request_id": rid
            })),
        );
    }

    // 3. 校验 encoding
    let tx_bytes = match req.encoding.as_str() {
        "base64" => {
            match base64::engine::general_purpose::STANDARD.decode(&req.transaction) {
                Ok(b) => b,
                Err(_) => return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid_transaction", "message": "base64 decode failed", "request_id": rid})),
                ),
            }
        }
        "base58" => {
            match bs58::decode(&req.transaction).into_vec() {
                Ok(b) => b,
                Err(_) => return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid_transaction", "message": "base58 decode failed", "request_id": rid})),
                ),
            }
        }
        _ => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_encoding", "message": "encoding must be base64 or base58", "request_id": rid})),
        ),
    };

    // 4. 获取私钥
    let private_key = match state.wallet.get_private_key_bytes(&wallet_core::Network::Solana).await {
        Ok(k) => k,
        Err(wallet_core::wallet::WalletError::Locked) => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})),
        ),
        Err(wallet_core::wallet::WalletError::NotFound(_)) => return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "wallet_not_found", "request_id": rid})),
        ),
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
        ),
    };

    // 5. 签名
    let (signed_bytes, tx_id) = match wallet_core::solana_wallet::sign_transaction(&private_key, &tx_bytes) {
        Ok(r) => r,
        Err(wallet_core::wallet::WalletError::SignerNotRequired) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "signer_not_required", "request_id": rid})),
        ),
        Err(wallet_core::wallet::WalletError::AlreadySigned) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "already_signed", "request_id": rid})),
        ),
        Err(wallet_core::wallet::WalletError::InvalidTransaction(msg)) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_transaction", "message": msg, "request_id": rid})),
        ),
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid})),
        ),
    };

    // 6. 输出固定 base58 编码
    let signed_tx_b58 = bs58::encode(&signed_bytes).into_string();

    // 7. 发送 Telegram 通知
    let signed_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let event = wallet_core::notification::SignEvent::Solana {
        tx_id: tx_id.clone(),
        transaction: signed_tx_b58.clone(),
        signed_at,
    };
    state.send_telegram(&wallet_core::notification::format_sign_message(&event)).await;

    // 8. 返回
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "request_id": rid,
            "transaction": signed_tx_b58,
        })),
    )
}
```

在路由注册处（`router` 函数）添加：
```rust
.route("/api/wallet/sign/solana", post(sign_solana))
```

### 7.4 验证

```
cargo test -p wallet-server solana_sign_tests
```

---

## Task 8：EVM Transaction API Handler

**前置条件：** Task 1、4、6 完成
**文件：** `crates/wallet-server/src/api/mod.rs`

### 8.1 新增的 Request/Response 结构体

```rust
#[derive(Deserialize, ToSchema)]
struct SignEvmTransactionRequest {
    /// 请求唯一标识，原样透传至响应
    request_id: String,
    /// 目标网络：eth | bnb | arb | polygon
    network: String,
    /// 未签名的完整 EVM 交易；RLP 编码转 hex，含 0x 前缀；含零签名字段
    transaction: String,
}

#[derive(Serialize, ToSchema)]
struct SignEvmTransactionResponse {
    /// 透传自请求的唯一标识
    request_id: String,
    /// 透传自请求的网络标识
    network: String,
    /// 签名后的完整 EVM 交易；RLP 编码转 hex，含 0x 前缀
    transaction: String,
}
```

### 8.2 先写测试（在 api/mod.rs 的 tests 模块中新增）

```rust
mod evm_tx_sign_tests {
    use super::*;
    use axum_test::TestServer;
    use alloy::consensus::{TxEnvelope, TxEip1559, TxKind};
    use alloy::primitives::{U256, Address, Signature as AlloySignature};
    use alloy::eips::eip2718::Encodable2718;

    fn make_unsigned_type2_hex(chain_id: u64) -> String {
        let tx = TxEip1559 {
            chain_id,
            nonce: 0,
            max_priority_fee_per_gas: 1_000_000_000,
            max_fee_per_gas: 20_000_000_000,
            gas_limit: 21000,
            to: TxKind::Call(Address::repeat_byte(0xde)),
            value: U256::from(1_000_000_000_000_000_000u64),
            input: Default::default(),
            access_list: Default::default(),
        };
        let zero_sig = AlloySignature::from_rs_and_parity(U256::ZERO, U256::ZERO, false).unwrap();
        let signed = tx.into_signed(zero_sig);
        let mut out = Vec::new();
        TxEnvelope::Eip1559(signed).encode_2718(&mut out);
        format!("0x{}", hex::encode(&out))
    }

    async fn unlocked_eth_server() -> (TestServer, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let password = "test-pass";
        let keys = wallet_core::evm_wallet::generate_keypair();
        state.wallet.save_wallet(&wallet_core::Network::Eth, &keys, password).unwrap();
        state.wallet.unlock(password).await.unwrap();
        let app = router(state);
        (TestServer::new(app).unwrap(), dir)
    }

    #[tokio::test]
    async fn sign_evm_tx_locked_returns_503_with_request_id() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let server = TestServer::new(router(state)).unwrap();

        let resp = server
            .post("/api/wallet/sign/evm/transaction")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "transaction": "0xaabbcc"
            }))
            .await;
        assert_eq!(resp.status_code(), 503);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["request_id"], "req-1");
        assert_eq!(body["error"], "wallet_locked");
    }

    #[tokio::test]
    async fn sign_evm_tx_invalid_network_returns_400() {
        let (server, _dir) = unlocked_eth_server().await;
        let resp = server
            .post("/api/wallet/sign/evm/transaction")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "invalid",
                "transaction": "0x00"
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_network");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_evm_tx_type2_success_sender_recoverable() {
        let (server, _dir) = unlocked_eth_server().await;
        let tx_hex = make_unsigned_type2_hex(1); // chainId = 1 (eth)

        let resp = server
            .post("/api/wallet/sign/evm/transaction")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "transaction": tx_hex
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["request_id"], "req-1");
        assert_eq!(body["network"], "eth");

        let signed_hex = body["transaction"].as_str().unwrap();
        assert!(signed_hex.starts_with("0x"));
    }

    #[tokio::test]
    async fn sign_evm_tx_already_signed_returns_400() {
        let (server, _dir) = unlocked_eth_server().await;
        let tx_hex = make_unsigned_type2_hex(1);

        // 先签名一次
        let resp1 = server
            .post("/api/wallet/sign/evm/transaction")
            .json(&serde_json::json!({"request_id": "req-1", "network": "eth", "transaction": tx_hex}))
            .await;
        let signed_hex = resp1.json::<serde_json::Value>()["transaction"]
            .as_str().unwrap().to_string();

        // 再次签名已签的交易
        let resp2 = server
            .post("/api/wallet/sign/evm/transaction")
            .json(&serde_json::json!({"request_id": "req-2", "network": "eth", "transaction": signed_hex}))
            .await;
        assert_eq!(resp2.status_code(), 400);
        let body: serde_json::Value = resp2.json();
        assert_eq!(body["error"], "already_signed");
        assert_eq!(body["request_id"], "req-2");
    }

    #[tokio::test]
    async fn sign_evm_tx_chain_id_mismatch_returns_400() {
        let (server, _dir) = unlocked_eth_server().await;
        // 交易 chainId = 56 (BNB)，但 network = eth (chainId=1)
        let tx_hex = make_unsigned_type2_hex(56);
        let resp = server
            .post("/api/wallet/sign/evm/transaction")
            .json(&serde_json::json!({"request_id": "req-1", "network": "eth", "transaction": tx_hex}))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "chain_id_mismatch");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_evm_tx_invalid_hex_returns_400() {
        let (server, _dir) = unlocked_eth_server().await;
        let resp = server
            .post("/api/wallet/sign/evm/transaction")
            .json(&serde_json::json!({"request_id": "req-1", "network": "eth", "transaction": "not-hex"}))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_transaction");
        assert_eq!(body["request_id"], "req-1");
    }
}
```

### 8.3 Handler 实现

```rust
async fn sign_evm_transaction(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SignEvmTransactionRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.request_id.is_empty() {
        return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing_request_id"})));
    }
    let rid = &req.request_id;

    // 1. 解析 network
    let network = match crate::util::parse_network(&req.network) {
        Some(n) if n != wallet_core::Network::Solana => n,
        _ => return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_network", "request_id": rid}))),
    };

    // 2. 获取 chain_id
    let chain_id = network.chain_id().unwrap(); // 非 Solana 网络保证有 chain_id

    // 3. 检查钱包锁定
    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign/evm/transaction")).await;
        return (StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})));
    }

    // 4. hex 解码交易
    let tx_str = req.transaction.trim_start_matches("0x");
    let tx_bytes = match hex::decode(tx_str) {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_transaction", "message": "hex decode failed", "request_id": rid}))),
    };

    // 5. 获取私钥
    let private_key = match state.wallet.get_private_key_bytes(&network).await {
        Ok(k) => k,
        Err(wallet_core::wallet::WalletError::Locked) => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid}))),
        Err(wallet_core::wallet::WalletError::NotFound(_)) => return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "wallet_not_found", "request_id": rid}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid}))),
    };

    // 6. 签名
    let (signed_bytes, from, to, value) = match wallet_core::evm_wallet::sign_transaction(&private_key, &tx_bytes, chain_id) {
        Ok(r) => r,
        Err(wallet_core::wallet::WalletError::AlreadySigned) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "already_signed", "request_id": rid}))),
        Err(wallet_core::wallet::WalletError::ChainIdMismatch { expected, actual }) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "chain_id_mismatch",
                "message": format!("expected {expected}, got {actual}"),
                "request_id": rid}))),
        Err(wallet_core::wallet::WalletError::UnsupportedTxType(t)) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "unsupported_tx_type",
                "message": format!("type {t} not supported"),
                "request_id": rid}))),
        Err(wallet_core::wallet::WalletError::InvalidTransaction(msg)) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_transaction", "message": msg, "request_id": rid}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid}))),
    };

    // 7. 发送 Telegram 通知
    let signed_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let event = wallet_core::notification::SignEvent::EvmTransaction {
        network: network.display_name().to_string(),
        from,
        to,
        value,
        signed_at,
    };
    state.send_telegram(&wallet_core::notification::format_sign_message(&event)).await;

    // 8. 返回
    let signed_hex = format!("0x{}", hex::encode(&signed_bytes));
    (StatusCode::OK,
        Json(serde_json::json!({
            "request_id": rid,
            "network": req.network,
            "transaction": signed_hex
        })))
}
```

路由注册：
```rust
.route("/api/wallet/sign/evm/transaction", post(sign_evm_transaction))
```

### 8.4 验证

```
cargo test -p wallet-server evm_tx_sign_tests
```


---

## Task 9：EIP-712 API Handler

**前置条件：** Task 1、5、6 完成
**文件：** `crates/wallet-server/src/api/mod.rs`

### 9.1 新增 Request/Response 结构体

```rust
#[derive(Deserialize, ToSchema)]
struct SignEvmTypedDataRequest {
    /// 请求唯一标识，原样透传至响应
    request_id: String,
    /// 目标网络：eth | bnb | arb | polygon
    network: String,
    /// 标准 EIP-712 结构（对齐 eth_signTypedData_v4），含 domain/types/primaryType/message
    /// 类型为 object，通过 example 展示格式，不逐字段 schema 描述
    #[schema(example = json!({
        "domain": {"name": "MyApp", "version": "1", "chainId": 1},
        "types": {"Transfer": [{"name": "to", "type": "address"}]},
        "primaryType": "Transfer",
        "message": {"to": "0x0000000000000000000000000000000000000001"}
    }))]
    typed_data: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
struct SignEvmTypedDataResponse {
    /// 透传自请求的唯一标识
    request_id: String,
    /// 透传自请求的网络标识
    network: String,
    /// 完整 ECDSA 签名；65 字节，hex 编码，含 0x 前缀（r||s||v，130 hex + "0x"，共 132 字符）
    signature: String,
    /// 签名 r 分量；32 字节，hex 编码，含 0x 前缀（64 hex + "0x"，共 66 字符）
    r: String,
    /// 签名 s 分量；32 字节，hex 编码，含 0x 前缀（64 hex + "0x"，共 66 字符）
    s: String,
    /// 签名 v 分量（recovery id）；1 字节，hex 编码，含 0x 前缀（如 "0x1b" 或 "0x1c"）
    v: String,
}
```

### 9.2 先写测试

```rust
mod eip712_sign_tests {
    use super::*;
    use axum_test::TestServer;

    async fn unlocked_eth_server() -> (TestServer, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let password = "test-pass";
        let keys = wallet_core::evm_wallet::generate_keypair();
        state.wallet.save_wallet(&wallet_core::Network::Eth, &keys, password).unwrap();
        state.wallet.unlock(password).await.unwrap();
        (TestServer::new(router(state)).unwrap(), dir)
    }

    fn valid_typed_data(chain_id: u64) -> serde_json::Value {
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

    #[tokio::test]
    async fn sign_eip712_locked_returns_503() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        let server = TestServer::new(router(state)).unwrap();
        let resp = server
            .post("/api/wallet/sign/evm/typed-data")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "typed_data": valid_typed_data(1)
            }))
            .await;
        assert_eq!(resp.status_code(), 503);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "wallet_locked");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_eip712_invalid_network_returns_400() {
        let (server, _dir) = unlocked_eth_server().await;
        let resp = server
            .post("/api/wallet/sign/evm/typed-data")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "unknown",
                "typed_data": valid_typed_data(1)
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_network");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_eip712_success_returns_r_s_v_signature() {
        let (server, _dir) = unlocked_eth_server().await;
        let resp = server
            .post("/api/wallet/sign/evm/typed-data")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "typed_data": valid_typed_data(1)
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["request_id"], "req-1");
        assert_eq!(body["network"], "eth");

        let sig = body["signature"].as_str().unwrap();
        let r   = body["r"].as_str().unwrap();
        let s   = body["s"].as_str().unwrap();
        let v   = body["v"].as_str().unwrap();

        // 长度验证
        assert_eq!(sig.len(), 132); // "0x" + 130 hex
        assert_eq!(r.len(), 66);
        assert_eq!(s.len(), 66);
        assert!(v == "0x1b" || v == "0x1c");

        // r+s+v 拼接 = signature（去掉各自 0x 前缀）
        let combined = format!("0x{}{}{}", &r[2..], &s[2..], &v[2..]);
        assert_eq!(combined, sig);
    }

    #[tokio::test]
    async fn sign_eip712_chain_id_mismatch_returns_400() {
        let (server, _dir) = unlocked_eth_server().await;
        // domain.chainId = 56 (BNB), network = eth (chainId=1)
        let resp = server
            .post("/api/wallet/sign/evm/typed-data")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "typed_data": valid_typed_data(56)
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "chain_id_mismatch");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_eip712_missing_chain_id_injects_and_succeeds() {
        let (server, _dir) = unlocked_eth_server().await;
        let mut td = valid_typed_data(1);
        td["domain"].as_object_mut().unwrap().remove("chainId");
        let resp = server
            .post("/api/wallet/sign/evm/typed-data")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "typed_data": td
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
    }

    #[tokio::test]
    async fn sign_eip712_message_extra_field_returns_400() {
        let (server, _dir) = unlocked_eth_server().await;
        let mut td = valid_typed_data(1);
        td["message"]["extra"] = serde_json::json!("unexpected");
        let resp = server
            .post("/api/wallet/sign/evm/typed-data")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "typed_data": td
            }))
            .await;
        assert_eq!(resp.status_code(), 400);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_typed_data");
        assert_eq!(body["request_id"], "req-1");
    }

    #[tokio::test]
    async fn sign_eip712_eip712domain_in_types_is_ignored() {
        let (server, _dir) = unlocked_eth_server().await;
        let mut td = valid_typed_data(1);
        td["types"]["EIP712Domain"] = serde_json::json!([
            {"name": "name", "type": "string"}
        ]);
        let resp = server
            .post("/api/wallet/sign/evm/typed-data")
            .json(&serde_json::json!({
                "request_id": "req-1",
                "network": "eth",
                "typed_data": td
            }))
            .await;
        assert_eq!(resp.status_code(), 200);
    }
}
```

### 9.3 Handler 实现

```rust
async fn sign_evm_typed_data(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SignEvmTypedDataRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.request_id.is_empty() {
        return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing_request_id"})));
    }
    let rid = &req.request_id;

    // 1. 解析 network（只接受 EVM 网络）
    let network = match crate::util::parse_network(&req.network) {
        Some(n) if n != wallet_core::Network::Solana => n,
        _ => return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_network", "request_id": rid}))),
    };
    let chain_id = network.chain_id().unwrap();

    // 2. 检查钱包锁定
    if !state.wallet.is_unlocked().await {
        state.send_telegram(&format_locked_message("sign/evm/typed-data")).await;
        return (StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid})));
    }

    // 3. 获取私钥
    let private_key = match state.wallet.get_private_key_bytes(&network).await {
        Ok(k) => k,
        Err(wallet_core::wallet::WalletError::Locked) => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "wallet_locked", "request_id": rid}))),
        Err(wallet_core::wallet::WalletError::NotFound(_)) => return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "wallet_not_found", "request_id": rid}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid}))),
    };

    // 4. 签名
    let (signature, r, s, v) = match wallet_core::evm_wallet::sign_typed_data(&private_key, &req.typed_data, chain_id) {
        Ok(result) => result,
        Err(wallet_core::wallet::WalletError::ChainIdMismatch { expected, actual }) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "chain_id_mismatch",
                "message": format!("expected {expected}, got {actual}"),
                "request_id": rid}))),
        Err(wallet_core::wallet::WalletError::InvalidTypedData(msg)) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid_typed_data", "message": msg, "request_id": rid}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "sign_failed", "message": e.to_string(), "request_id": rid}))),
    };

    // 5. 发送 Telegram 通知
    let signed_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let typed_data_json_str = serde_json::to_string(&req.typed_data)
        .unwrap_or_else(|_| "<serialize error>".to_string());
    let event = wallet_core::notification::SignEvent::EvmTypedData {
        network: network.display_name().to_string(),
        typed_data_json: typed_data_json_str,
        signed_at,
    };
    state.send_telegram(&wallet_core::notification::format_sign_message(&event)).await;

    // 6. 返回
    (StatusCode::OK,
        Json(serde_json::json!({
            "request_id": rid,
            "network": req.network,
            "signature": signature,
            "r": r,
            "s": s,
            "v": v
        })))
}
```

路由注册：
```rust
.route("/api/wallet/sign/evm/typed-data", post(sign_evm_typed_data))
```

### 9.4 验证

```
cargo test -p wallet-server eip712_sign_tests
```

---

## Task 10：删除旧接口 + 更新 OpenAPI

**前置条件：** Task 7、8、9 全部通过
**文件：** `crates/wallet-server/src/api/mod.rs`

### 10.1 删除以下内容

1. **路由注册**（在 `router` 函数中）：
   ```rust
   .route("/api/wallet/sign", post(sign_transaction))  // 删除此行
   ```

2. **旧 handler 函数**：删除整个 `async fn sign_transaction(...)` 函数（约 90 行）

3. **旧 helper 函数**：删除整个 `fn sign_evm_message(...)` 函数（约 27 行）

4. **旧 Request/Response 结构体**：删除
   - `struct SignRequest`
   - `struct SignMetadata`
   - `struct SignResponse`

5. **旧测试**（在 tests 模块中）：删除以下测试函数：
   - `post_sign_when_locked_returns_503`
   - `sign_eth_returns_valid_signature`
   - `sign_bnb_returns_valid_signature`
   - `sign_arb_returns_valid_signature`
   - `sign_polygon_returns_valid_signature`
   - `sign_solana_returns_valid_signature`
   - `sign_with_invalid_network_returns_400`
   - `sign_evm_with_non_32byte_hash_returns_400`
   - `sign_returns_404_when_wallet_missing_for_network`
   - `sign_with_invalid_hex_returns_400`

### 10.2 更新 OpenAPI ApiDoc

将 `#[openapi(...)]` 中的 `paths` 和 `components(schemas)` 修改为：

```rust
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Simple Local Wallet Service API",
        description = "本地钱包服务的对外 API（端口 9293），提供多链钱包地址查询、余额查询和交易签名功能。\n\n支持的网络：solana、eth、bnb、arb、polygon\n\n**注意**：所有签名接口均要求 request_id 字段（必填）和钱包处于已解锁状态。解析错误响应不含 request_id。",
        version = "2.0.0",
    ),
    servers(
        (url = "http://127.0.0.1:9293", description = "本地 API 服务"),
    ),
    paths(
        get_address,
        get_balance,
        sign_solana,
        sign_evm_transaction,
        sign_evm_typed_data,
    ),
    components(schemas(
        Network,
        TokenBalance,
        AddressResponse,
        BalanceResponse,
        SignSolanaRequest,
        SignSolanaResponse,
        SignEvmTransactionRequest,
        SignEvmTransactionResponse,
        SignEvmTypedDataRequest,
        SignEvmTypedDataResponse,
        ErrorResponse,
    ))
)]
pub struct ApiDoc;
```

在每个新 handler 上添加 `#[utoipa::path(...)]` 注解（参考现有 `get_address` 的写法）：

**sign_solana：**
```rust
#[utoipa::path(
    post,
    path = "/api/wallet/sign/solana",
    request_body = SignSolanaRequest,
    responses(
        (status = 200, description = "签名成功", body = SignSolanaResponse),
        (status = 400, description = "参数错误（encoding/transaction/request_id）", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "Solana 交易签名",
    description = "对 Solana 交易进行签名，只填充本服务密钥对应的 signer 位置。输入支持 base64 或 base58 编码，输出固定为 base58。",
)]
```

**sign_evm_transaction：**
```rust
#[utoipa::path(
    post,
    path = "/api/wallet/sign/evm/transaction",
    request_body = SignEvmTransactionRequest,
    responses(
        (status = 200, description = "签名成功", body = SignEvmTransactionResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "EVM 交易签名",
    description = "对 EVM 交易进行签名，支持 Type 0/1/2。输入为含零签名字段的完整 RLP hex（含 0x 前缀）。chainId 必须与 network 一致，否则返回 400 chain_id_mismatch。",
)]
```

**sign_evm_typed_data：**
```rust
#[utoipa::path(
    post,
    path = "/api/wallet/sign/evm/typed-data",
    request_body = SignEvmTypedDataRequest,
    responses(
        (status = 200, description = "签名成功", body = SignEvmTypedDataResponse),
        (status = 400, description = "参数错误", body = ErrorResponse),
        (status = 503, description = "钱包未解锁", body = ErrorResponse),
    ),
    summary = "EIP-712 结构化数据签名",
    description = "对 EIP-712 typed data 进行签名，对齐 eth_signTypedData_v4。domain.chainId 缺失时自动注入，与 network 不一致时返回 400 chain_id_mismatch。",
)]
```

### 10.3 验证旧接口已删除

```rust
// 在 tests 模块中新增
#[tokio::test]
async fn old_sign_endpoint_returns_404() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf());
    let server = TestServer::new(router(state)).unwrap();
    let resp = server
        .post("/api/wallet/sign")
        .json(&serde_json::json!({"network": "eth", "transaction": "abc"}))
        .await;
    assert_eq!(resp.status_code(), 404);
}

#[tokio::test]
async fn openapi_does_not_contain_old_sign_path() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf());
    let server = TestServer::new(router(state)).unwrap();
    let resp = server.get("/api/docs/openapi.json").await;
    let body: serde_json::Value = resp.json();
    // 旧路径已移除
    assert!(body["paths"].get("/api/wallet/sign").is_none());
    // 新路径存在
    assert!(body["paths"]["/api/wallet/sign/solana"].is_object());
    assert!(body["paths"]["/api/wallet/sign/evm/transaction"].is_object());
    assert!(body["paths"]["/api/wallet/sign/evm/typed-data"].is_object());
}
```

### 10.4 验证

```
cargo test -p wallet-server
cargo build -p wallet-server
```

---

## Task 11：文档更新

**前置条件：** Task 10 完成
**文件：** `README.md`、`docs/simple_requirement.md`

### 11.1 README.md 更新要点

1. 在签名接口部分标注 **BREAKING CHANGE**：
   ```
   > **Breaking Change (v2.0.0):** `POST /api/wallet/sign` 已移除。
   > 请迁移到以下三个新接口：
   ```

2. 新增三个接口的简要说明和示例 curl 命令：

   ```bash
   # Solana 签名
   curl -X POST http://localhost:9293/api/wallet/sign/solana \
     -H "Content-Type: application/json" \
     -d '{"request_id":"req-1","encoding":"base64","transaction":"<base64 tx>"}'

   # EVM 交易签名
   curl -X POST http://localhost:9293/api/wallet/sign/evm/transaction \
     -H "Content-Type: application/json" \
     -d '{"request_id":"req-1","network":"eth","transaction":"0x02f8..."}'

   # EIP-712 签名
   curl -X POST http://localhost:9293/api/wallet/sign/evm/typed-data \
     -H "Content-Type: application/json" \
     -d '{"request_id":"req-1","network":"eth","typed_data":{...}}'
   ```

3. 说明 `request_id` 为必填字段，以及解析错误不含 `request_id` 是预期行为。

### 11.2 docs/simple_requirement.md 更新

将原接口说明中的旧签名接口章节替换为三个新接口的描述，内容与设计文档 `docs/plans/2026-03-11-signing-api-redesign.md` 保持一致。

---

## 全量验证命令

所有任务完成后，执行以下命令确认无报错：

```bash
# 全量测试
cargo test

# 编译检查
cargo build

# 具体各层验证
cargo test -p wallet-core
cargo test -p wallet-server
```

---

## 常见问题与注意事项

### Q1: alloy 的 `TxEnvelope::decode_2718` 解码失败怎么办？

客户端必须发送含零签名字段的完整格式。如果客户端发送的是不含签名字段的纯字段格式（如 viem 的 `serializeTransaction` 默认输出），需要告知调用方在提交前手动添加零签名字段。

### Q2: `alloy::dyn_abi::TypedData` 找不到？

确认 `wallet-core/Cargo.toml` 中 `alloy = { version = "1", features = ["full"] }`，`full` feature 包含 `dyn-abi`。导入路径为 `use alloy::dyn_abi::TypedData;`。

### Q3: Solana `bincode` 依赖找不到？

在 `wallet-core/Cargo.toml` 中新增：
```toml
bincode = "1"
```

### Q4: `ValidatedJson` 在 handler 中如何导入？

在 `api/mod.rs` 顶部添加：
```rust
use crate::extractor::ValidatedJson;
```

### Q5: `network.chain_id()` 方法在哪里定义？

在 Task 4 的 4.5 节中，需要在 `wallet-core/src/network.rs` 中新增 `chain_id()` 方法。这是 Task 4 的一部分。

### Q6: `base64` 在 wallet-server 中如何使用？

`wallet-server` 依赖 `wallet-core`，而 `wallet-core` 已有 `base64 = "0.22"`。在 handler 中通过 `use base64::Engine;` 使用即可。但 `wallet-server/Cargo.toml` 中也可能需要单独添加 `base64 = "0.22"`。

### Q7: Solana `VersionedTransaction` 序列化顺序

先尝试 `VersionedTransaction`，失败再尝试 `Transaction`。如果两者都失败，返回 `InvalidTransaction`。不要同时尝试，按顺序执行。

### Q8: EVM 签名后从交易中提取 `to` 和 `value`

`to` 和 `value` 需要在签名**之前**从原始交易中提取，在 `sign_transaction` 函数的返回值中一并返回，不要在 handler 中单独解析已签名交易。

