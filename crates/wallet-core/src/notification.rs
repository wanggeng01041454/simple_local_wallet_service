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
        assert!(msg.to_lowercase().contains("lock"));
    }

    #[test]
    fn telegram_client_new_stores_credentials() {
        let client = TelegramClient::new("bot_token".to_string(), "chat_id".to_string());
        assert_eq!(client.bot_token, "bot_token");
        assert_eq!(client.chat_id, "chat_id");
    }
}
