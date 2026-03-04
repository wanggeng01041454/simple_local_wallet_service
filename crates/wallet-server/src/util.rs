use wallet_core::network::Network;

pub fn parse_network(s: &str) -> Option<Network> {
    match s {
        "solana" => Some(Network::Solana),
        "eth" => Some(Network::Eth),
        "bnb" => Some(Network::Bnb),
        "arb" => Some(Network::Arb),
        _ => None,
    }
}
