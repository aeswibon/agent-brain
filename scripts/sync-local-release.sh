#!/usr/bin/env bash
# Download the latest GitHub release tag, sign (macOS), and link MCP — no MCP recheck wait.
# Does not force package git pulls (those stay on interval_hours, default 24h).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${AGENT_BRAIN_BIN:-$HOME/.local/bin/agent-brain}"

if [[ -x "$BIN" ]]; then
  exec "$BIN" update --force --mcp-only
fi

exec "$ROOT/scripts/install.sh" --global
