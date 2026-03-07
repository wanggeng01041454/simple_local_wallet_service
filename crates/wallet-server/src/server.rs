use crate::state::AppState;
use http::HeaderValue;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;

pub async fn run(data_dir: PathBuf) -> anyhow::Result<()> {
    let state = AppState::new(data_dir);

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9292".parse::<HeaderValue>().unwrap())
        .allow_methods([http::Method::GET, http::Method::POST])
        .allow_headers([http::header::CONTENT_TYPE]);

    let api_router = crate::api::router(state.clone()).layer(cors);
    let admin_router = crate::admin::router(state.clone());

    let api_addr: SocketAddr = "127.0.0.1:9293".parse()?;
    let admin_addr: SocketAddr = "127.0.0.1:9292".parse()?;

    tracing::info!("API server listening on {api_addr}");
    tracing::info!("Admin server listening on http://{admin_addr}");

    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    let admin_listener = tokio::net::TcpListener::bind(admin_addr).await?;

    tokio::try_join!(
        axum::serve(api_listener, api_router),
        axum::serve(admin_listener, admin_router),
    )?;

    Ok(())
}
