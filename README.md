# agent-brain

Fast, local MCP server that routes each turn to the right **agents, skills, rules, and memory** under a strict token budget.

Rust is the brain; Cursor/Claude are the hands.

**Full guide:** [docs/USAGE.md](docs/USAGE.md)

## Do I start the MCP manually?

**No.** After `agent-brain install --global`, **Cursor starts `agent-brain serve` automatically** when you open the editor. You only run `serve` yourself when debugging.

## Quick start (new laptop)

```bash
# 1. Install binary + write ~/.cursor/mcp.json
curl -fsSL https://raw.githubusercontent.com/aeswibon/agent-brain/main/scripts/install.sh | bash -s -- --global

# 2. Restart Cursor, enable agent-brain under Settings → MCP

# 3. (Optional) Install ECC or other skill packages
agent-brain add affaan-m/ecc
```

That's it. Open Cursor in **Agent mode** — the editor starts agent-brain, indexes on boot, and the installed rule requires `route_task` each turn.

## What runs automatically

| Action | Who does it |
|--------|-------------|
| Start MCP server | Cursor spawns `agent-brain serve` |
| Index skills/rules/memory | agent-brain on MCP startup |
| Route each turn | Agent calls `route_task` — **blocked by Cursor hooks** until called |
| Persist decisions | Agent calls `store_memory` at task end |

`install --global` writes MCP config, a Cursor rule, and **hooks** (`~/.cursor/hooks.json`) that deny other tools until `route_task` runs each turn.

CLI is only for one-time setup (`install`, optional `add`) or maintenance (`package update`).

## Other agents

The MCP server is host-agnostic. **Cursor** has a one-command installer. **Claude Code / Codex / Claude Desktop** work with the same binary — add it to their MCP config manually; skills under `~/.claude/` and `~/.codex/` are already indexed. Host-specific installers come later.

## Install options

**Release binary** (from [Releases](https://github.com/aeswibon/agent-brain/releases)):

```bash
# download the binary for your OS, then:
chmod +x agent-brain-*
mv agent-brain-* ~/.local/bin/agent-brain
agent-brain install --global
```

**From source** (requires Rust + git):

```bash
cargo install --git https://github.com/aeswibon/agent-brain --locked agent-brain
agent-brain install --global
agent-brain add affaan-m/ecc   # optional packages
```

## Commands

| Command | Description |
|---------|-------------|
| `agent-brain install --global` | Write `~/.cursor/mcp.json` (one-time) |
| `agent-brain add <owner/repo>` | Install a GitHub skills/agents package |
| `agent-brain package list\|update\|remove` | Manage installed packages |
| `agent-brain index` | Force reindex (optional — also runs on MCP start) |
| `agent-brain serve` | Manual MCP server (debug only) |

## MCP config

`agent-brain install --global` writes:

```json
{
  "mcpServers": {
    "agent-brain": {
      "command": "/Users/you/.local/bin/agent-brain",
      "args": ["serve"],
      "env": { "RUST_LOG": "agent_brain=info" }
    }
  }
}
```

Cursor spawns this process automatically — you do not need a terminal running `serve`.

## Packages

```bash
agent-brain add affaan-m/ecc
agent-brain package update ecc
```

Clones to `~/.agent_brain/packages/` and indexes skills, agents, rules, and commands.

## Data directory

`~/.agent_brain/` (override with `AGENT_BRAIN_HOME`). First MCP start downloads the embedding model (~90MB).

## Development

```bash
cargo test --release -p agent-brain
cargo build --release -p agent-brain
```

## Releases

See [CHANGELOG.md](CHANGELOG.md). Tags `v*` publish platform binaries with changelog-based release notes.

## License

MIT
