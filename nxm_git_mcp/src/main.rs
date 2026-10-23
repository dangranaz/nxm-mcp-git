use anyhow::Context as _;
use clap::Parser;
use nxm_git_mcp::server::GitServer;
use std::net::SocketAddr;

/// Serve MCP Streamable HTTP on this address (e.g. `127.0.0.1:7281`)
/// instead of stdio. Falls back to `NXM_GIT_MCP_HTTP_ADDR`; stdio when unset.
#[derive(Parser)]
#[command(
    name = "nxm_git_mcp",
    version,
    about = "Serve the nxm-git tools over MCP"
)]
struct Args {
    /// Serve MCP Streamable HTTP on this address instead of stdio.
    #[arg(long, value_name = "ADDR")]
    http: Option<SocketAddr>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    let server = GitServer::new()?;

    match resolve_http_addr(args.http)? {
        Some(addr) => {
            let app = nxm_git_mcp::http::router(server);
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .with_context(|| format!("binding HTTP listener on {addr}"))?;
            tracing::info!(
                target: "nxm::git::mcp::http",
                %addr,
                route = nxm_git_mcp::http::GIT_MCP_ROUTE,
                "serving MCP Streamable HTTP (tools: 7 read + 5 write)"
            );
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .context("MCP Streamable HTTP server")?;
        }
        None => {
            tracing::info!(target: "nxm::git::mcp", "serving MCP on stdio (tools: 7 read + 5 write)");
            let service = rmcp::serve_server(server, rmcp::transport::stdio()).await?;
            service.waiting().await?;
        }
    }
    Ok(())
}

/// Transport selection: the `--http` flag wins over `NXM_GIT_MCP_HTTP_ADDR`;
/// `None` means stdio.
fn resolve_http_addr(flag: Option<SocketAddr>) -> anyhow::Result<Option<SocketAddr>> {
    if let Some(addr) = flag {
        return Ok(Some(addr));
    }
    match std::env::var_os("NXM_GIT_MCP_HTTP_ADDR") {
        Some(raw) => Ok(Some(
            raw.to_string_lossy()
                .parse()
                .context("parsing NXM_GIT_MCP_HTTP_ADDR as ip:port")?,
        )),
        None => Ok(None),
    }
}

/// Wait for the HTTP shutdown trigger: Ctrl-C (or SIGTERM on unix).
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => tracing::warn!(
                target: "nxm::git::mcp::http",
                error = %e,
                "SIGTERM watcher unavailable; Ctrl-C still works"
            ),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!(target: "nxm::git::mcp::http", "shutdown signal received; draining connections");
}
