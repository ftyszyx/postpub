use std::{env, net::SocketAddr, sync::Arc};

use anyhow::Context;
use postpub_api::build_router;
use postpub_core::AppContext;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let addr = read_bind_addr()?;
    let context = Arc::new(AppContext::new("postpub-api", env!("CARGO_PKG_VERSION")));
    context
        .bootstrap()
        .context("failed to prepare postpub workspace")?;
    let app = build_router(context);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind to {addr}"))?;

    tracing::info!("postpub web launcher listening on http://{addr}");

    axum::serve(listener, app)
        .await
        .context("axum server exited with error")?;

    Ok(())
}

fn read_bind_addr() -> anyhow::Result<SocketAddr> {
    let value = env::var("POSTPUB_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    value
        .parse()
        .with_context(|| format!("invalid POSTPUB_BIND_ADDR value: {value}"))
}
