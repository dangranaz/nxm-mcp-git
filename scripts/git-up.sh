#!/usr/bin/env bash
# git-up.sh — start the nxm-git MCP server on HTTP :7281 (manual, stays
# live until you run git-down.sh or reboot). Idempotent: no-op if already up.
#
# Serves MCP Streamable HTTP at /mcp, backed by the nxm-git core lib.
# Shared by all clients — one process.
set -euo pipefail

ADDR="127.0.0.1:7281"
PORT="7281"
REPO="$HOME/Projects/private/nxm-git"
BIN="$REPO/target/release/nxm_git_mcp"
PIDFILE="$HOME/.nxm/git-mcp.pid"
LOG="$HOME/.nxm/git-mcp.log"

log() { printf '\033[1;34m[git-up]\033[0m %s\n' "$*"; }
err() { printf '\033[1;31m[git-up] ERROR:\033[0m %s\n' "$*" >&2; }

# Already up?
if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
    log "already up on $ADDR (nothing to do)"
    exit 0
fi

mkdir -p "$HOME/.nxm"

# Atomic start lock: `mkdir` fails if dir exists.
LOCKDIR="$HOME/.nxm/git-up.lock"
if ! mkdir "$LOCKDIR" 2>/dev/null; then
    HOLDER="$(cat "$LOCKDIR/pid" 2>/dev/null || true)"
    if [[ -n "$HOLDER" ]] && kill -0 "$HOLDER" 2>/dev/null; then
        log "another start is in progress (pid $HOLDER); nothing to do"
        exit 0
    fi
    log "found stale start-lock; reclaiming"
    rm -rf "$LOCKDIR"
    mkdir "$LOCKDIR" 2>/dev/null || { log "could not acquire start-lock; deferring"; exit 0; }
fi
echo "$$" > "$LOCKDIR/pid"
trap 'rm -rf "$LOCKDIR"' EXIT

# Re-check under lock.
if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
    log "already up on $ADDR (started concurrently; nothing to do)"
    exit 0
fi

[[ -x "$BIN" ]] || { err "binary not found: $BIN — run: cargo build --release -p nxm-git-mcp"; exit 1; }

log "starting nxm-git MCP on $ADDR"
NXM_GIT_CONFIG_DIR="$HOME/.nxm" RUST_LOG="${RUST_LOG:-info}" \
    nohup "$BIN" --http "$ADDR" >>"$LOG" 2>&1 &
echo $! > "$PIDFILE"

# Wait for the port (server bind almost instant)
for i in $(seq 1 10); do
    if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
        log "✅ up on http://$ADDR/mcp (pid $(cat "$PIDFILE"), log: $LOG)"
        exit 0
    fi
    sleep 1
done
err "did not come up within 10s — check $LOG"
exit 1