use crate::network::Network;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TokenBalance {
    pub token: String,
    pub balance: String,
}

// EVM token contract addresses
struct EvmTokens {
    native_symbol: &'static str,
    usdt: &'static str,
    usdc: &'static str,
}

fn evm_tokens(network: &Network) -> EvmTokens {
    match network {
        Network::Eth => EvmTokens {
            native_symbol: "ETH",
            usdt: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            usdc: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        },
        Network::Bnb => EvmTokens {
            native_symbol: "BNB",
            usdt: "0x55d398326f99059fF775485246999027B3197955",
            usdc: "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d",
        },
        Network::Arb => EvmTokens {
            native_symbol: "ETH",
            usdt: "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9",
            usdc: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
        },
        Network::Polygon => EvmTokens {
            native_symbol: "POL",
            usdt: "0xc2132D05D31c914a87C6611C10748AEb04B58e8F",
            usdc: "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
        },
        _ => unreachable!(),
    }
}

// Solana SPL token mints
const SOLANA_USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
const SOLANA_USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

/// Query all token balances (native + USDT + USDC) for a given network and address.
pub async fn query_balances(rpc_url: &str, network: &Network, address: &str) -> Vec<TokenBalance> {
    match network {
        Network::Solana => query_solana_balances(rpc_url, address).await,
        _ => query_evm_balances(rpc_url, network, address).await,
    }
}

async fn query_evm_balances(rpc_url: &str, network: &Network, address: &str) -> Vec<TokenBalance> {
    let tokens = evm_tokens(network);
    let client = reqwest::Client::new();

    let native_fut = evm_native_balance(client.clone(), rpc_url, address);

    // BNB chain USDT/USDC use 18 decimals; all others use 6
    let (usdt_decimals, usdc_decimals) = match network {
        Network::Bnb => (18, 18),
        _ => (6, 6),
    };

    let usdt_fut = evm_erc20_balance(client.clone(), rpc_url, address, tokens.usdt, usdt_decimals);
    let usdc_fut = evm_erc20_balance(client.clone(), rpc_url, address, tokens.usdc, usdc_decimals);

    let (native, usdt, usdc) = tokio::join!(native_fut, usdt_fut, usdc_fut);

    vec![
        TokenBalance {
            token: tokens.native_symbol.to_string(),
            balance: native.unwrap_or_else(|_| "0".to_string()),
        },
        TokenBalance {
            token: "USDT".to_string(),
            balance: usdt.unwrap_or_else(|_| "0".to_string()),
        },
        TokenBalance {
            token: "USDC".to_string(),
            balance: usdc.unwrap_or_else(|_| "0".to_string()),
        },
    ]
}

async fn evm_native_balance(
    client: reqwest::Client,
    rpc_url: &str,
    address: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getBalance",
        "params": [address, "latest"],
        "id": 1
    });
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let hex_str = resp["result"].as_str().ok_or("missing result")?;
    let wei = u128::from_str_radix(hex_str.trim_start_matches("0x"), 16)?;
    Ok(format_units(wei, 18))
}

async fn evm_erc20_balance(
    client: reqwest::Client,
    rpc_url: &str,
    address: &str,
    contract: &str,
    decimals: u8,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // balanceOf(address) selector = 0x70a08231, address padded to 32 bytes
    let addr_clean = address.trim_start_matches("0x");
    let data = format!("0x70a08231{:0>64}", addr_clean);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": contract, "data": data}, "latest"],
        "id": 1
    });
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let hex_str = resp["result"].as_str().ok_or("missing result")?;
    let clean = hex_str.trim_start_matches("0x");
    if clean.is_empty() || clean.chars().all(|c| c == '0') {
        return Ok("0".to_string());
    }
    let value = u128::from_str_radix(clean, 16)?;
    Ok(format_units(value, decimals))
}

async fn query_solana_balances(rpc_url: &str, address: &str) -> Vec<TokenBalance> {
    let client = reqwest::Client::new();

    let native_fut = solana_native_balance(client.clone(), rpc_url, address);
    let usdt_fut = solana_spl_balance(client.clone(), rpc_url, address, SOLANA_USDT_MINT);
    let usdc_fut = solana_spl_balance(client.clone(), rpc_url, address, SOLANA_USDC_MINT);

    let (native, usdt, usdc) = tokio::join!(native_fut, usdt_fut, usdc_fut);

    vec![
        TokenBalance {
            token: "SOL".to_string(),
            balance: native.unwrap_or_else(|_| "0".to_string()),
        },
        TokenBalance {
            token: "USDT".to_string(),
            balance: usdt.unwrap_or_else(|_| "0".to_string()),
        },
        TokenBalance {
            token: "USDC".to_string(),
            balance: usdc.unwrap_or_else(|_| "0".to_string()),
        },
    ]
}

async fn solana_native_balance(
    client: reqwest::Client,
    rpc_url: &str,
    address: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "getBalance",
        "params": [address],
        "id": 1
    });
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let lamports = resp["result"]["value"].as_u64().ok_or("missing value")?;
    Ok(format_units(lamports as u128, 9))
}

async fn solana_spl_balance(
    client: reqwest::Client,
    rpc_url: &str,
    address: &str,
    mint: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "getTokenAccountsByOwner",
        "params": [
            address,
            {"mint": mint},
            {"encoding": "jsonParsed"}
        ],
        "id": 1
    });
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    let accounts = resp["result"]["value"]
        .as_array()
        .ok_or("missing value array")?;

    parse_solana_token_balance(accounts)
}

fn parse_solana_token_balance(
    accounts: &[serde_json::Value],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if accounts.is_empty() {
        return Ok("0".to_string());
    }

    let mut total = 0u128;
    let mut decimals = None;

    for account in accounts {
        let token_amount = &account["account"]["data"]["parsed"]["info"]["tokenAmount"];
        let amount = token_amount["amount"].as_str().ok_or("missing amount")?;
        let account_decimals = token_amount["decimals"]
            .as_u64()
            .ok_or("missing decimals")? as u8;

        total = total
            .checked_add(amount.parse::<u128>()?)
            .ok_or("token amount overflow")?;

        match decimals {
            Some(existing) if existing != account_decimals => {
                return Err("inconsistent token decimals".into())
            }
            None => decimals = Some(account_decimals),
            _ => {}
        }
    }

    Ok(format_units(total, decimals.unwrap_or(0)))
}

fn format_units(value: u128, decimals: u8) -> String {
    if value == 0 {
        return "0".to_string();
    }
    let divisor = 10u128.pow(decimals as u32);
    let whole = value / divisor;
    let frac = value % divisor;
    if frac == 0 {
        return whole.to_string();
    }
    let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
    let trimmed = frac_str.trim_end_matches('0');
    format!("{}.{}", whole, trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_units_zero() {
        assert_eq!(format_units(0, 18), "0");
    }

    #[test]
    fn format_units_whole_number() {
        assert_eq!(format_units(1_000_000_000_000_000_000, 18), "1");
    }

    #[test]
    fn format_units_with_fraction() {
        assert_eq!(format_units(1_500_000_000_000_000_000, 18), "1.5");
    }

    #[test]
    fn format_units_small_fraction() {
        assert_eq!(format_units(1_234_567, 6), "1.234567");
    }

    #[test]
    fn format_units_trailing_zeros_trimmed() {
        assert_eq!(format_units(1_230_000, 6), "1.23");
    }

    #[test]
    fn format_units_sol_lamports() {
        // 1.5 SOL = 1_500_000_000 lamports
        assert_eq!(format_units(1_500_000_000, 9), "1.5");
    }

    #[test]
    fn parse_solana_token_balance_sums_multiple_accounts() {
        let accounts = vec![
            serde_json::json!({
                "account": {
                    "data": {
                        "parsed": {
                            "info": {
                                "tokenAmount": {
                                    "amount": "1250000",
                                    "decimals": 6
                                }
                            }
                        }
                    }
                }
            }),
            serde_json::json!({
                "account": {
                    "data": {
                        "parsed": {
                            "info": {
                                "tokenAmount": {
                                    "amount": "750000",
                                    "decimals": 6
                                }
                            }
                        }
                    }
                }
            }),
        ];

        assert_eq!(parse_solana_token_balance(&accounts).unwrap(), "2");
    }
}
