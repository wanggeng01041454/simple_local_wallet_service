use tracing::{error, info};

// ---- 工具函数 ----

/// 转义 Telegram Markdown v1 特殊字符：_ * [ ] ( ) ~ ` > # + - = | { } . !
pub fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match c {
            '_' | '*' | '[' | ']' | '(' | ')' | '~' | '`' | '>' | '#' | '+'
            | '-' | '=' | '|' | '{' | '}' | '.' | '!' => {
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
            let txid_escaped = escape_markdown(tx_id.as_deref().unwrap_or("N/A"));
            // Escape user data first, then truncate so the "..." marker stays unescaped
            let tx_full_escaped = escape_markdown(transaction);
            let tx_display = truncate_str(&tx_full_escaped, 60);
            format!(
                "*[Solana 签名]*\nTxID: `{}`\n交易: `{}`\n时间: {}",
                txid_escaped, tx_display, signed_at
            )
        }
        SignEvent::EvmTransaction { network, from, to, value, signed_at } => {
            let from_escaped = escape_markdown(from);
            let to_escaped = escape_markdown(to.as_deref().unwrap_or("N/A (contract creation)"));
            let value_escaped = escape_markdown(value);
            format!(
                "*[EVM 签名]*\n网络: {}\nFrom: `{}`\nTo: `{}`\nValue: {} wei\n时间: {}",
                network, from_escaped, to_escaped, value_escaped, signed_at
            )
        }
        SignEvent::EvmTypedData { network, typed_data_json, signed_at } => {
            // Escape user data first, then add truncation marker (template text, not escaped)
            let escaped_data = escape_markdown(typed_data_json);
            let display = if escaped_data.chars().count() > 500 {
                let s: String = escaped_data.chars().take(500).collect();
                format!("{}...（已截断）", s)
            } else {
                escaped_data
            };
            format!(
                "*[EIP-712 签名]*\n网络: {}\n数据: `{}`\n时间: {}",
                network, display, signed_at
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

    #[test]
    fn escape_markdown_escapes_extended_set() {
        // Spot-check the newly added characters
        assert_eq!(escape_markdown("(a)"), "\\(a\\)");
        assert_eq!(escape_markdown("a.b"), "a\\.b");
        assert_eq!(escape_markdown("a!b"), "a\\!b");
        assert_eq!(escape_markdown("a~b"), "a\\~b");
        assert_eq!(escape_markdown("a>b"), "a\\>b");
        assert_eq!(escape_markdown("a|b"), "a\\|b");
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
        assert!(msg.contains("...（已截断）"));
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
