#!/usr/bin/env bash
# git-down.sh — stop the nxm-git MCP server started by git-up.sh.
set -euo pipefail

PORT="7281"
PIDFILE="$HOME/.nxm/git-mcp.pid"

log() { printf '\033[1;34m[git-down]\033[0m %s\n' "$*"; }

stopped=0
# Prefer the pidfile; fall back to whoever listens on the port.
if [[ -f "$PIDFILE" ]] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    pid="$(cat "$PIDFILE")"
    log "stopping pid $pid (SIGTERM)"
    kill "$pid" 2>/dev/null || true
    stopped=1
fi
for pid in $(lsof -nP -tiTCP:"$PORT" -sTCP:LISTEN 2>/dev/null || true); do
    log "stopping listener pid $pid on :$PORT (SIGTERM)"
    kill "$pid" 2>/dev/null || true
    stopped=1
done

sleep 1
rm -f "$PIDFILE"
if [[ "$stopped" -eq 1 ]]; then
    log "✅ stopped"
else
    log "nothing running on :$PORT"
fi