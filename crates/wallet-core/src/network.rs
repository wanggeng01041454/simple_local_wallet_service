use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Solana,
    Eth,
    Bnb,
    Arb,
    Polygon,
}

impl Network {
    pub fn all() -> &'static [Network] {
        &[Network::Solana, Network::Eth, Network::Bnb, Network::Arb, Network::Polygon]
    }

    pub fn wallet_filename(&self) -> &'static str {
        match self {
            Network::Solana => "solana.enc",
            Network::Eth => "eth.enc",
            Network::Bnb => "bnb.enc",
            Network::Arb => "arb.enc",
            Network::Polygon => "polygon.enc",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Network::Solana => "Solana",
            Network::Eth => "Ethereum",
            Network::Bnb => "BNB Chain",
            Network::Arb => "Arbitrum",
            Network::Polygon => "Polygon",
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
