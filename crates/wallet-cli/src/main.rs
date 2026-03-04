use anyhow::Context;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("local-wallet")
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local-wallet")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("local_wallet=info".parse()?)
                .add_directive("wallet_server=info".parse()?)
                .add_directive("wallet_core=info".parse()?),
        )
        .init();

    let data_dir = data_dir();
    tracing::info!("data directory: {}", data_dir.display());
    std::fs::create_dir_all(&data_dir).context("failed to create data directory")?;

    wallet_server::server::run(data_dir).await
}
