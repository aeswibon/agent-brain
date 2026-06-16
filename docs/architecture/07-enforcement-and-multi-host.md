# 7. Enforcement and multi-host integration

## Summary

**Cursor** gets hard enforcement via hooks. **Other hosts** (OpenCode, Claude Code, VS Code, Claude Desktop) get MCP registration plus instruction files — the agent is told to call `route_task`, but nothing blocks other tools if it disobeys.

## What we built

### Cursor hooks (`hooks/route_gate.py`)

Installed by `agent-brain install --global` into `~/.cursor/hooks.json`:

| Hook | Behavior |
|------|----------|
| `beforeMCPExecution` | Track `route_task` success / failure |
| `preToolUse` | Deny tools until routed for current user turn |

State persisted in `~/.agent_brain/hooks/route_state.json`.

### Gate scope (`AGENT_BRAIN_ROUTE_GATE_SCOPE`)

| Value | Behavior |
|-------|----------|
| `brain_mcp` (default) | Only gate agent-brain MCP tools until `route_task`; Shell/Read keep working if MCP offline |
| `all` | Legacy strict mode — gate every tool |

**Why scoped gate:** v0.7.1+ — MCP disconnect should not freeze the entire session. Offline cooldown (`AGENT_BRAIN_ROUTE_OFFLINE_SECS`) eventually allows proceed-with-warning.

### Grace and stale routing

- **Grace period** after failed `route_task` — avoid infinite lock if MCP errors
- **Stale route timeout** — re-route if last success too old within same turn

### Permissions (`~/.cursor/permissions.json`)

`install --global` adds `agent-brain:*` to MCP allowlist so Cursor CLI agents skip per-session approval prompts.

### Multi-host install (`host_install.rs`)

| Host | Command | Config file |
|------|---------|-------------|
| Cursor | `install --global` | `~/.cursor/mcp.json` + hooks |
| Claude Desktop | `install --claude-desktop` | `claude_desktop_config.json` |
| VS Code | `install --vscode [--global]` | `.vscode/mcp.json` or user mcp.json |
| Claude Code | `install --claude-code [--global]` | `.mcp.json` or `~/.claude.json` |
| OpenCode | `install --opencode [--global]` | `opencode.json` |
| All | `install --all --global` | Above combined |

Each host gets an **instruction file** on first install (e.g. `~/.config/opencode/agent-brain.md`, `.claude/agent-brain.md`) describing required `route_task` usage.

OpenCode JSON shape uses `mcp.agent-brain.type: local` and `command: [binary, serve]`.

### Doctor

`agent-brain doctor` reports Cursor MCP path, hooks, codesign; shows OpenCode/Claude Code global MCP status. `--fix` realigns Cursor and can run `install --all`.

## Why hooks are Cursor-only

Cursor exposes **preToolUse** / **beforeMCPExecution** with deny semantics. OpenCode and Claude Code do not offer an equivalent “block all tools until X” hook API agent-brain can target today.

**Implication:** Multi-host parity is **config + instructions**, not **enforcement parity**.

## Alternatives considered

### Rely on rules only (no hooks)

**Rejected:** Insufficient — models skip optional steps under pressure.

### Gate all tools always (`scope=all`)

**Shipped as opt-in:** Too brittle when MCP restarts or user needs emergency shell.

### Custom Cursor extension instead of hooks

**Rejected:** Hooks are the supported 2025+ mechanism; extension maintenance burden.

### OpenCode fork / patch

**Rejected:** Upstream instruction file + MCP is maintainable; patch is not.

### Workspace-only MCP (no global)

**Default Cursor install is global:** Skills are user-level; project-only MCP would miss `~/.cursor/skills` index context unless cwd bootstrap covers it.

## Trade-offs

- **Non-Cursor hosts:** Discipline depends on model following `agent-brain.md` — verify with `last-route.md` / MCP logs.
- **Hook false positives:** Tool name matching must track Cursor MCP naming (`mcp_agent-brain_route_task`, etc.).
- **Python hook dependency:** Cursor runs hook scripts with system Python; kept stdlib-only.

## For senior engineers and principal architects

### Enforcement is a product feature, not an implementation detail

On Cursor, **routing is mandatory** before other tools (within gate scope). On OpenCode/Claude, routing is **advisory**. This asymmetry is the largest **host parity gap** in the system — not retrieval quality.

PE implication: if your org standardizes on OpenCode, you inherit **model compliance risk**. Mitigations today:

- Instruction file on install (`agent-brain.md`)
- `doctor` verifying MCP registration
- Operators spot-checking `~/.agent_brain/logs/last-route.md`

Long-term fix requires **host APIs with deny semantics** — we cannot polyfill that in MCP alone.

### Scoped gate (`brain_mcp`) rationale

`scope=all` blocked Shell/Read when MCP flapped — developers could not recover without disabling hooks. **`brain_mcp` scope** says: “You must route before calling agent-brain tools; other tools remain available if MCP is down.”

This is a **degraded-mode** design familiar from distributed systems: prefer partial availability over total lockout.

### Offline grace and stale route

| Mechanism | Purpose |
|-----------|---------|
| `AGENT_BRAIN_ROUTE_OFFLINE_SECS` | After MCP failures, allow proceed with warning |
| Stale route timeout | Re-route within long turns if context shifted |

Without grace, a single MCP crash **bricks the agent session** — unacceptable for daily driver IDE use.

### Multi-host install matrix

`install --all --global` exists because PEs asked for **one command** to align Cursor + OpenCode + Claude. Each host has different JSON shapes (`opencode.json` vs `mcp.json`); `host_install.rs` centralizes that knowledge.

**macOS codesign** is part of “install works” — unsigned binaries die under taskgated when Cursor spawns MCP (see [10](10-concurrency-and-performance.md)).

### Failure modes

| Symptom | Cause | Action |
|---------|-------|--------|
| Infinite deny loop | `route_task` errors every turn | Fix MCP binary; check `doctor` |
| Agent never routes (OpenCode) | No hooks | Reinforce instruction file; custom lint |
| Wrong tool name in gate | Cursor renamed MCP tools | Update `route_gate.py` patterns |
| Hooks missing after update | Install not re-run | `agent-brain install --global` |

### Questions a PE should ask

1. What **% of traffic** is on hook-enforced Cursor vs advisory hosts?
2. Is **degraded proceed without route** acceptable under MCP outage?
3. Do security policies allow **global MCP** in `~/.cursor/mcp.json`?
4. Who maintains **hook scripts** when Cursor hook schema changes?

## Further reading

- [host-integration.md](../host-integration.md)
- [route_gate.py](../../agent-brain/hooks/route_gate.py)
- [install.rs](../../agent-brain/src/install.rs)
