#!/usr/bin/env bash
# nxm-git installer (build-from-source, private product).
#
#   ./install.sh
#
# nxm-git is a PRIVATE product: there is no public GitHub release to download.
# This installer builds the server from the local checkout, installs the binary
# to ~/.local/bin, seeds the server config if missing, and registers nxm-git as
# an MCP server on every supported client it finds on this machine.
#
# It is idempotent: re-running updates the binary and leaves existing config in
# place (client configs are backed up before any edit).
#
# Overrides (env vars):
#   NXM_GIT_BIN_DIR     install dir for the binary (default: $HOME/.local/bin)
#   NXM_GIT_ALLOWED_ROOT default allowed_roots entry when seeding the config
#                        (default: $HOME/Projects/private)
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="nxm_git_mcp"
BIN_DIR="${NXM_GIT_BIN_DIR:-$HOME/.local/bin}"
CONFIG="$HOME/.nxm/git-mcp.toml"
ALLOWED_ROOT="${NXM_GIT_ALLOWED_ROOT:-$HOME/Projects/private}"
URL="http://127.0.0.1:7281/mcp"

say()  { printf '\033[1;34m==>\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33mNOTE:\033[0m %s\n' "$1"; }
err()  { printf '\033[1;31mERROR:\033[0m %s\n' "$1" >&2; exit 1; }

# --- 1. build ---------------------------------------------------------------
command -v cargo >/dev/null 2>&1 || err "cargo is required to build nxm-git"
say "building release binary (this may take a minute)"
( cd "$REPO_DIR" && cargo build --release --workspace >/dev/null )

SRC_BIN="$REPO_DIR/target/release/$BIN_NAME"
[ -x "$SRC_BIN" ] || err "built binary not found at $SRC_BIN"

# --- 2. install binary ------------------------------------------------------
mkdir -p "$BIN_DIR"
install -m 0755 "$SRC_BIN" "$BIN_DIR/$BIN_NAME"
say "installed binary → $BIN_DIR/$BIN_NAME"

case ":$PATH:" in
  *":$BIN_DIR:"*) : ;;
  *) warn "add $BIN_DIR to your PATH:  export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

# --- 3. seed server config (safety perimeter) -------------------------------
if [ -f "$CONFIG" ]; then
  say "config exists, leaving as-is → $CONFIG"
else
  mkdir -p "$(dirname "$CONFIG")"
  cat > "$CONFIG" <<EOF
# nxm-git server config — safety perimeter.
# The server refuses to operate on any repository outside these roots.
allowed_roots = [
  "$ALLOWED_ROOT",
]
EOF
  say "seeded config → $CONFIG (allowed_roots: $ALLOWED_ROOT)"
fi

# --- 4. register as MCP on supported clients --------------------------------
# Each edit backs up the file first. Registration is best-effort: a client that
# is not installed is skipped, never an error.
backup() { [ -f "$1" ] && cp "$1" "$1.bak.$(date +%Y%m%d-%H%M%S)"; }

register_json_url() { # file, "needs timeout?"
  # Adds/updates an "nxm-git" entry inside the top-level "mcpServers" object of
  # a client config, using jq when available. url-style HTTP server.
  local file="$1" with_timeout="$2"
  [ -f "$file" ] || return 0
  command -v jq >/dev/null 2>&1 || { warn "jq not found; register nxm-git in $file by hand"; return 0; }
  backup "$file"
  local entry
  if [ "$with_timeout" = "yes" ]; then
    entry="{\"url\": \"$URL\", \"timeout\": 60000}"
  else
    entry="{\"url\": \"$URL\"}"
  fi
  jq --argjson e "$entry" '.mcpServers["nxm-git"] = $e' "$file" > "$file.tmp" && mv "$file.tmp" "$file"
  say "registered nxm-git → $file"
}

# kiro-cli (mcpServers, with timeout)
register_json_url "$HOME/.kiro/settings/mcp.json" yes
# pi.dev (mcpServers, no timeout)
register_json_url "$HOME/.pi/agent/mcp.json" no

# opencode (official v2 config: `mcp` object in opencode.jsonc,
# {type:remote,url,enabled}). The legacy top-level mcp.json is NOT written —
# newer opencode releases key off opencode.jsonc, and keeping two sources risks
# a stale one silently winning after an upgrade.
OC="$HOME/.config/opencode/opencode.jsonc"
if [ -f "$OC" ] && command -v jq >/dev/null 2>&1; then
  if jq empty "$OC" >/dev/null 2>&1; then
    backup "$OC"
    jq '.mcp["nxm-git"] = {"type":"remote","url":"'"$URL"'","enabled":true}' "$OC" > "$OC.tmp" \
      && mv "$OC.tmp" "$OC"
    say "registered nxm-git → $OC"
  else
    warn "$OC has comments jq cannot parse; add nxm-git under \"mcp\" by hand"
  fi
elif [ ! -f "$OC" ]; then
  warn "opencode config not found ($OC) — skipping opencode registration"
fi

# --- 5. done ----------------------------------------------------------------
say "installed. Start the server with:  ./scripts/git-up.sh   (or: nxm-mcp up)"
warn "freebuff/Manicode does not consume external MCP servers — not applicable."
