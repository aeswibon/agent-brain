#!/usr/bin/env bash
# Download the latest GitHub release, sign (macOS), and link MCP — no interval wait.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${AGENT_BRAIN_BIN:-$HOME/.local/bin/agent-brain}"

if [[ -x "$BIN" ]]; then
  exec "$BIN" update --force
fi

exec "$ROOT/scripts/install.sh" --global
