use std::sync::Arc;

use crate::server::GitServer;
use axum::Router;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tracing::info;

/// Route of the MCP endpoint.
pub const GIT_MCP_ROUTE: &str = "/mcp";

/// Build the MCP Streamable HTTP router for the git tools.
///
/// One `server` (built once by the caller) serves every HTTP session: each
/// session gets a cheap [`Clone`] sharing the same server state.
/// Serve it with `axum::serve(listener, router(server))`: MCP then answers on
/// [`GIT_MCP_ROUTE`].
///
/// # Example
/// ```
/// use nxm_git_mcp::server::GitServer;
/// let server = GitServer::new().unwrap();
/// let router = nxm_git_mcp::http::router(server);
/// // axum::serve(listener, router) now answers MCP on /mcp.
/// ```
pub fn router(server: GitServer) -> Router {
    info!(target: "nxm::git::mcp::http", route = GIT_MCP_ROUTE, "ENTER router");
    let server = Arc::new(server);
    // Stateless mode: direct JSON responses (no SSE, no session-id), matching
    // nxm-memory's transport. git_status is pure request->response, so
    // sessions/streaming are pure overhead — stateless is lighter and uniform.
    let config = StreamableHttpServerConfig {
        stateful_mode: false,
        ..Default::default()
    };
    let service = StreamableHttpService::new(
        move || Ok((*server).clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let router = Router::new().route_service(GIT_MCP_ROUTE, service);
    info!(target: "nxm::git::mcp::http", route = GIT_MCP_ROUTE, stateful = false, "EXIT router");
    router
}
