use crate::state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;

pub async fn run(data_dir: PathBuf) -> anyhow::Result<()> {
    let state = AppState::new(data_dir);

    let api_router = crate::api::router(state.clone());
    let admin_router = crate::admin::router(state.clone());

    let api_addr: SocketAddr = "127.0.0.1:9293".parse()?;
    let admin_addr: SocketAddr = "0.0.0.0:9292".parse()?;

    tracing::info!("API server listening on {api_addr}");
    tracing::info!("Admin server listening on {admin_addr}");

    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    let admin_listener = tokio::net::TcpListener::bind(admin_addr).await?;

    tokio::try_join!(
        axum::serve(api_listener, api_router),
        axum::serve(admin_listener, admin_router),
    )?;

    Ok(())
}
