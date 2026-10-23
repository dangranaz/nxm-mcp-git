use std::fs;
use std::process::Command;

use anyhow::{Context as _, Result};
use http_body_util::{BodyExt as _, Full};
use hyper::body::{Bytes, Incoming};
use hyper::client::conn::http1;
use hyper::Request;
use hyper_util::rt::TokioIo;
use nxm_git_mcp::http::GIT_MCP_ROUTE;
use nxm_git_mcp::server::GitServer;
use serde_json::{json, Value};
use tempfile::TempDir;

/// Serve the HTTP transport on an ephemeral port with a given server factory; returns `host:port`.
async fn spawn_fallback_http<F>(server_factory: F) -> String
where
    F: Fn() -> GitServer + Send + Sync + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral listener");
    let addr = listener.local_addr().expect("local address");
    let server = server_factory();
    let app = nxm_git_mcp::http::router(server);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("test http server failed: {e}");
        }
    });
    addr.to_string()
}

/// POST one JSON-RPC message on its own connection; returns the raw response.
async fn post(
    addr: &str,
    body: &Value,
    session: Option<&str>,
) -> Result<hyper::Response<Incoming>> {
    let stream = tokio::net::TcpStream::connect(addr)
        .await
        .context("connecting to test server")?;
    let (mut sender, conn) = http1::handshake(TokioIo::new(stream))
        .await
        .context("HTTP handshake")?;
    tokio::spawn(conn);
    let mut builder = Request::builder()
        .method("POST")
        .uri(GIT_MCP_ROUTE)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(session) = session {
        builder = builder.header("mcp-session-id", session);
    }
    sender
        .send_request(builder.body(Full::new(Bytes::from(body.to_string())))?)
        .await
        .context("sending JSON-RPC message")
}

/// Read the full response body and parse it as a single JSON-RPC message
/// (stateless transport: direct JSON, not SSE).
async fn read_json(resp: hyper::Response<Incoming>) -> Result<Value> {
    let bytes = resp
        .into_body()
        .collect()
        .await
        .context("reading body")?
        .to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    // Stateless returns application/json; be tolerant if a data: prefix appears.
    let payload = text
        .lines()
        .map(|l| l.strip_prefix("data: ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str(payload.trim()).context("response body as JSON")
}

/// Real MCP handshake over HTTP (stateless): `initialize` returns serverInfo
/// directly as JSON; no session id is issued.
async fn handshake(addr: &str) -> Result<()> {
    let init = json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "http_tool_test", "version": "0.0.0"}
        }
    });
    let resp = post(addr, &init, None).await?;
    anyhow::ensure!(
        resp.status().is_success(),
        "initialize: HTTP {}",
        resp.status()
    );
    let message = read_json(resp).await?;
    let server_name = message["result"]["serverInfo"]["name"]
        .as_str()
        .context("initialize result carries serverInfo.name")?;
    anyhow::ensure!(!server_name.is_empty(), "serverInfo.name must not be empty");
    Ok(())
}

/// `tools/call` over HTTP (stateless); returns the decoded `content[0].text`.
async fn call_tool(addr: &str, id: i64, name: &str, arguments: Value) -> Result<String> {
    let message = json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    });
    let resp = post(addr, &message, None).await?;
    anyhow::ensure!(
        resp.status().is_success(),
        "tools/call {name}: HTTP {}",
        resp.status()
    );
    let message = read_json(resp).await?;
    let result = &message["result"];
    anyhow::ensure!(
        result["isError"].as_bool() != Some(true),
        "tools/call {name} returned isError"
    );
    result["content"][0]["text"]
        .as_str()
        .map(str::to_owned)
        .with_context(|| format!("tools/call {name}: content[0].text"))
}

/// `tools/call` over HTTP expecting a structured tool error (`isError`):
/// returns the decoded `content[0].text` payload.
async fn call_tool_err(addr: &str, id: i64, name: &str, arguments: Value) -> Result<String> {
    let message = json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    });
    let resp = post(addr, &message, None).await?;
    anyhow::ensure!(
        resp.status().is_success(),
        "tools/call {name}: HTTP {}",
        resp.status()
    );
    let message = read_json(resp).await?;
    let result = &message["result"];
    anyhow::ensure!(
        result["isError"].as_bool() == Some(true),
        "tools/call {name}: expected isError"
    );
    result["content"][0]["text"]
        .as_str()
        .map(str::to_owned)
        .with_context(|| format!("tools/call {name}: content[0].text"))
}

/// Run git in `dir`, asserting success (test setup only).
fn git_ok(dir: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test]
async fn http_handshake_and_git_status() -> Result<()> {
    let td = TempDir::new().expect("failed to create temp dir");
    let repo_path = td.path().to_path_buf();
    let repo_path_clone = repo_path.clone();

    let addr = spawn_fallback_http(move || {
        GitServer::new_with_allowed_roots(vec![repo_path_clone.clone()])
    })
    .await;

    handshake(&addr).await?;

    let output = Command::new("git")
        .current_dir(&repo_path)
        .arg("init")
        .output()
        .expect("failed to run git init");
    assert!(output.status.success());
    Command::new("git")
        .current_dir(&repo_path)
        .arg("config")
        .arg("user.name")
        .arg("Test")
        .output()
        .expect("failed to set user.name");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("config")
        .arg("user.email")
        .arg("test@example.com")
        .output()
        .expect("failed to set user.email");
    fs::write(repo_path.join("staged.txt"), "content").expect("failed to write file");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("add")
        .arg("staged.txt")
        .output()
        .expect("failed to git add");
    fs::write(repo_path.join("staged.txt"), "modified content").expect("failed to modify");
    fs::write(repo_path.join("untracked.txt"), "world").expect("failed to write file");

    let text = call_tool(
        &addr,
        1,
        "git_status",
        json!({
            "repo_path": repo_path.to_string_lossy()
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse Status")?;
    let staged = v["staged"].as_array().expect("staged is array");
    let unstaged = v["unstaged"].as_array().expect("unstaged is array");
    let untracked = v["untracked"].as_array().expect("untracked is array");
    assert_eq!(staged.len(), 1);
    assert_eq!(
        staged[0].as_str().expect("staged[0] is string"),
        "staged.txt"
    );
    assert_eq!(unstaged.len(), 1);
    assert_eq!(
        unstaged[0].as_str().expect("unstaged[0] is string"),
        "staged.txt"
    );
    assert_eq!(untracked.len(), 1);
    assert_eq!(
        untracked[0].as_str().expect("untracked[0] is string"),
        "untracked.txt"
    );
    Ok(())
}

#[tokio::test]
async fn http_tools_list_has_fourteen_tools() -> Result<()> {
    let td = TempDir::new().expect("failed to create temp dir");
    let repo_path = td.path().to_path_buf();
    let repo_path_clone = repo_path.clone();

    let addr = spawn_fallback_http(move || {
        GitServer::new_with_allowed_roots(vec![repo_path_clone.clone()])
    })
    .await;

    handshake(&addr).await?;

    let message = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list",
        "params": {}
    });
    let resp = post(&addr, &message, None).await?;
    anyhow::ensure!(
        resp.status().is_success(),
        "tools/list: HTTP {}",
        resp.status()
    );
    let message = read_json(resp).await?;

    let tools = message["result"]["tools"]
        .as_array()
        .context("tools/list result missing tools array")?;

    assert_eq!(tools.len(), 14, "Expected 14 tools, got {}", tools.len());

    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool name is string"))
        .collect();

    let expected = [
        "git_status",
        "git_diff",
        "git_log",
        "git_branches",
        "git_show",
        "git_blame",
        "git_scan_forbidden",
        "git_commit",
        "git_create_branch",
        "git_create_integration_branch",
        "git_worktree_add",
        "git_switch",
        "git_push",
        "git_pr_create",
    ];
    for expected_name in expected {
        assert!(
            names.contains(&expected_name),
            "Missing tool: {}",
            expected_name
        );
    }

    println!("✅ All 14 tools present: {:?}", names);
    Ok(())
}

#[tokio::test]
async fn http_git_pr_create_rejects_invalid_base_with_structured_error() -> Result<()> {
    let td = TempDir::new().expect("failed to create temp dir");
    let repo_path = td.path().join("repo");
    fs::create_dir_all(&repo_path).expect("failed to create repo dir");
    git_ok(&repo_path, &["init", "-b", "main"]);

    let allowed_root = td.path().to_path_buf();
    let addr =
        spawn_fallback_http(move || GitServer::new_with_allowed_roots(vec![allowed_root.clone()]))
            .await;

    handshake(&addr).await?;

    // An option-like base is refused before `gh` could ever run: the client
    // gets the same structured payload shape as the write tools.
    let text = call_tool_err(
        &addr,
        1,
        "git_pr_create",
        json!({
            "repo_path": repo_path.to_string_lossy(),
            "title": "feat: add git_pr_create",
            "body": "Opened by the MCP tool.",
            "base": "--draft"
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse PrError payload")?;
    assert_eq!(v["kind"], "error");
    let message = v["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("invalid git name"),
        "unexpected message: {message}"
    );
    Ok(())
}

#[tokio::test]
async fn http_write_tools_commit_and_gated_push() -> Result<()> {
    let td = TempDir::new().expect("failed to create temp dir");
    let repo_path = td.path().join("repo");
    let remote_path = td.path().join("origin.git");
    fs::create_dir_all(&repo_path).expect("failed to create repo dir");
    git_ok(&repo_path, &["init", "-b", "main"]);
    git_ok(&repo_path, &["config", "user.name", "Test"]);
    git_ok(&repo_path, &["config", "user.email", "test@example.com"]);
    git_ok(&repo_path, &["config", "commit.gpgsign", "false"]);
    git_ok(td.path(), &["init", "--bare", "-b", "main", "origin.git"]);
    git_ok(
        &repo_path,
        &["remote", "add", "origin", &remote_path.to_string_lossy()],
    );

    let allowed_root = td.path().to_path_buf();
    let addr =
        spawn_fallback_http(move || GitServer::new_with_allowed_roots(vec![allowed_root.clone()]))
            .await;

    handshake(&addr).await?;

    fs::write(repo_path.join("a.txt"), "hello").expect("failed to write file");
    git_ok(&repo_path, &["add", "a.txt"]);

    // Commit on protected main: denied without confirm, structured error.
    let text = call_tool_err(
        &addr,
        1,
        "git_commit",
        json!({
            "repo_path": repo_path.to_string_lossy(),
            "message": "add a"
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse denied CommitResult")?;
    assert_eq!(v["kind"], "policy_denied");
    assert_eq!(v["op"], "Commit");
    assert_eq!(v["branch"], "main");

    // Git never ran: HEAD must still be unborn.
    let out = Command::new("git")
        .current_dir(&repo_path)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    assert!(!out.status.success(), "commit ran despite the denial");

    // With confirm: commits.
    let text = call_tool(
        &addr,
        2,
        "git_commit",
        json!({
            "repo_path": repo_path.to_string_lossy(),
            "message": "add a",
            "confirm": true
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse CommitResult")?;
    assert_eq!(v["branch"], "main");
    assert!(!v["sha"].as_str().unwrap_or("").is_empty(), "sha in result");

    // Push to main: denied by default, structured error.
    let text = call_tool_err(
        &addr,
        3,
        "git_push",
        json!({
            "repo_path": repo_path.to_string_lossy()
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse denied PushResult")?;
    assert_eq!(v["kind"], "policy_denied");
    assert_eq!(v["op"], "Push");
    assert_eq!(v["branch"], "main");

    // With confirm: pushes to the bare temp remote.
    let text = call_tool(
        &addr,
        4,
        "git_push",
        json!({
            "repo_path": repo_path.to_string_lossy(),
            "confirm": true
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse PushResult")?;
    assert_eq!(v["branch"], "main");
    assert_eq!(v["remote"], "origin");
    assert_eq!(v["forced"], false);

    // The bare temp remote actually received main.
    let out = Command::new("git")
        .current_dir(&remote_path)
        .args(["show-ref"])
        .output()?;
    assert!(String::from_utf8_lossy(&out.stdout).contains("refs/heads/main"));
    Ok(())
}

#[tokio::test]
async fn http_git_scan_forbidden_detects_private_traces() -> Result<()> {
    let td = TempDir::new().expect("failed to create temp dir");
    let repo_path = td.path().to_path_buf();
    let repo_path_clone = repo_path.clone();

    let addr = spawn_fallback_http(move || {
        GitServer::new_with_allowed_roots(vec![repo_path_clone.clone()])
    })
    .await;

    handshake(&addr).await?;

    let output = Command::new("git")
        .current_dir(&repo_path)
        .arg("init")
        .output()
        .expect("failed to run git init");
    assert!(output.status.success());
    Command::new("git")
        .current_dir(&repo_path)
        .arg("config")
        .arg("user.name")
        .arg("Test")
        .output()
        .expect("failed to set user.name");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("config")
        .arg("user.email")
        .arg("test@example.com")
        .output()
        .expect("failed to set user.email");

    fs::write(
        repo_path.join("secret.txt"),
        "This contains /Users/secret/path",
    )
    .expect("failed to write file");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("add")
        .arg("secret.txt")
        .output()
        .expect("failed to git add");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("commit")
        .arg("-m")
        .arg("add secret")
        .output()
        .expect("failed to git commit");

    let text = call_tool(
        &addr,
        1,
        "git_scan_forbidden",
        json!({
            "repo_path": repo_path.to_string_lossy()
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse ScanResult")?;

    assert!(
        !v["clean"].as_bool().unwrap(),
        "Should detect forbidden pattern"
    );
    let hits = v["hits"].as_array().expect("hits is array");
    assert!(!hits.is_empty(), "Should have at least one hit");
    let hit = &hits[0];
    assert_eq!(hit["file"].as_str().unwrap(), "secret.txt");
    assert!(hit["content"].as_str().unwrap().contains("/Users/"));

    println!("✅ Forbidden pattern detected: {:?}", hits);
    Ok(())
}

#[tokio::test]
async fn http_git_scan_forbidden_clean_repo() -> Result<()> {
    let td = TempDir::new().expect("failed to create temp dir");
    let repo_path = td.path().to_path_buf();
    let repo_path_clone = repo_path.clone();

    let addr = spawn_fallback_http(move || {
        GitServer::new_with_allowed_roots(vec![repo_path_clone.clone()])
    })
    .await;

    handshake(&addr).await?;

    let output = Command::new("git")
        .current_dir(&repo_path)
        .arg("init")
        .output()
        .expect("failed to run git init");
    assert!(output.status.success());
    Command::new("git")
        .current_dir(&repo_path)
        .arg("config")
        .arg("user.name")
        .arg("Test")
        .output()
        .expect("failed to set user.name");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("config")
        .arg("user.email")
        .arg("test@example.com")
        .output()
        .expect("failed to set user.email");

    fs::write(repo_path.join("clean.txt"), "This is a clean file").expect("failed to write file");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("add")
        .arg("clean.txt")
        .output()
        .expect("failed to git add");
    Command::new("git")
        .current_dir(&repo_path)
        .arg("commit")
        .arg("-m")
        .arg("add clean")
        .output()
        .expect("failed to git commit");

    let text = call_tool(
        &addr,
        1,
        "git_scan_forbidden",
        json!({
            "repo_path": repo_path.to_string_lossy()
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse ScanResult")?;

    assert!(v["clean"].as_bool().unwrap(), "Should be clean");
    let hits = v["hits"].as_array().expect("hits is array");
    assert!(hits.is_empty(), "Should have no hits");

    println!("✅ Clean repo correctly reported as CLEAN");
    Ok(())
}

#[tokio::test]
async fn http_create_integration_branch_then_worktree_add() -> Result<()> {
    let td = TempDir::new().expect("failed to create temp dir");
    let repo_path = td.path().join("repo");
    fs::create_dir_all(&repo_path).expect("failed to create repo dir");
    git_ok(&repo_path, &["init", "-b", "main"]);
    git_ok(&repo_path, &["config", "user.name", "Test"]);
    git_ok(&repo_path, &["config", "user.email", "test@example.com"]);
    git_ok(&repo_path, &["config", "commit.gpgsign", "false"]);
    git_ok(&repo_path, &["commit", "--allow-empty", "-m", "root"]);
    git_ok(&repo_path, &["checkout", "-b", "develop"]);
    git_ok(&repo_path, &["commit", "--allow-empty", "-m", "on develop"]);
    // Activate the workstream guards so the integration name is validated.
    fs::write(
        repo_path.join(".nxm-git-policy.toml"),
        "protected_branches = [\"main\"]\n\
         integration_branches = [\"integration/*\"]\n\
         develop_direct_commit = \"confirm\"\n\
         develop_branch = \"develop\"\n\
         work_branch_prefix = \"work/\"\n",
    )
    .expect("failed to write policy");

    let allowed_root = td.path().to_path_buf();
    let addr =
        spawn_fallback_http(move || GitServer::new_with_allowed_roots(vec![allowed_root.clone()]))
            .await;

    handshake(&addr).await?;

    // Step one: integration/ws rooted at develop.
    let text = call_tool(
        &addr,
        1,
        "git_create_integration_branch",
        json!({
            "repo_path": repo_path.to_string_lossy(),
            "workstream": "ws"
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse BranchResult")?;
    assert_eq!(v["branch"], "integration/ws");

    // Step two: its isolated worktree under .worktrees/ws.
    let text = call_tool(
        &addr,
        2,
        "git_worktree_add",
        json!({
            "repo_path": repo_path.to_string_lossy(),
            "workstream": "ws"
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse WorktreeResult")?;
    assert_eq!(v["branch"], "integration/ws");
    assert_eq!(v["path"], ".worktrees/ws");
    assert!(
        repo_path.join(".worktrees/ws").is_dir(),
        "worktree dir must exist"
    );

    // Adding the worktree for a non-existent branch is refused (ordering).
    let text = call_tool_err(
        &addr,
        3,
        "git_worktree_add",
        json!({
            "repo_path": repo_path.to_string_lossy(),
            "workstream": "missing"
        }),
    )
    .await?;
    let v: Value = serde_json::from_str(&text).context("parse error payload")?;
    assert_eq!(v["kind"], "error");

    Ok(())
}
