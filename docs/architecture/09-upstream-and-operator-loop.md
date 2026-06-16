# 9. Upstream federation and operator loop

## Summary

**Upstream federation (v0.8)** delegates specialized work to other MCP servers. **Operator loop (v0.10+)** adds human-in-the-loop skill promotion, memory GC, weekly digest, and CI eval — tools for maintaining a healthy brain over months.

## Upstream federation

### What we built

- Config block `upstream_mcp` in `~/.agent_brain/config.yaml` (up to 2 stdio servers)
- MCP tool **`route_to_mcp`** — explicit forward to upstream tool with truncation
- **`suggested_tools`** in `route_task` — keyword-ranked hints from cached upstream `list_tools` index
- Secrets via `${ENV}` templates resolved from keychain/env (`secrets.rs`)
- Logging in `retrieval_log` with `phase=upstream_call`

### Why explicit `route_to_mcp`, not auto-proxy every turn

**Control:** Upstream tools may be slow, costly, or side-effecting. Router suggests; agent or user logic decides.

**Alternatives considered:**

| Approach | Verdict |
|----------|---------|
| Embed all upstream tools in one MCP | Namespace collision, huge tool list |
| Auto-call upstream on every route | Latency + cost explosion |
| HTTP gateway to MCP fleet | Extra infra; stdio child processes match Cursor model |

## Operator loop (v0.10 / v0.11)

### promote_to_skill

1. MCP **`promote_to_skill`** stages `SKILL.md` draft under `~/.agent_brain/staging/`
2. CLI `promote list|approve|reject` — human gate before copying to `.cursor/skills/`

**Why human approval:** Agent-generated skills are often wrong; staging prevents polluting global skill library.

**Alternative:** Auto-write skills — rejected after early tests showed low quality and naming collisions.

### memory gc

`agent-brain memory gc [--apply] [--force]`:

- Candidates: low `context_weights`, stale session digests
- Protects: negative polarity, `apply_when`, high-confidence user facts
- Archives to `facts_archive` with reason buckets (`low_signal`, `stale_session_digest`)
- Thresholds configurable in `config.yaml` (`memory_gc.stale_days`, `very_stale_days`)

**Why archive, not delete:** Operators can audit what left active memory.

### digest --weekly

`operator_digest.rs` aggregates `retrieval_log` + context feedback for last N days — route volume, cache hit rate, low-weight items.

**Why:** Visibility without SQL; complements `last-route.md` per-turn view.

### eval --ci

Golden queries against seeded **memory and skill** fixtures; **Recall@3 ≥ 0.85** gate per suite in CI.

**Why dual suites:** Memory-only eval missed skill routing regressions — the product USP is skill/agent selection, not just fact recall. Skill goldens include decoy skills to catch “always return something” failures.

**Alternative:** LLM-judged eval — expensive, flaky in CI.

## Schema v6 tables

- `skill_staging` — promotion workflow state
- `facts_archive` — GC audit trail

## For senior engineers and principal architects

### Operator loop = long-term SLO maintenance

Shipping retrieval without **GC + eval + digest** is like shipping metrics without alerts. The operator loop answers: “Is my brain still healthy after 6 months of session ingest and skill pack updates?”

| Tool | SLO dimension |
|------|----------------|
| `eval --ci` | Routing correctness (regression) |
| `memory gc` | Memory SNR (signal-to-noise) |
| `digest --weekly` | Usage, cache hit rate, feedback |
| `promote` | Controlled skill library growth |

### Upstream federation boundary

`route_to_mcp` is **explicit delegation** — the router suggests upstream tools but does not auto-invoke them. Reasons:

- Upstream tools may have **side effects** (deploy, send email)
- Latency stacks multiplicatively per turn
- Tool namespace explosion if merged into one MCP

This matches **API gateway** patterns: route table + optional forward, not transparent proxy of entire backend.

### Promotion human gate

Auto-writing `SKILL.md` from agent output polluted libraries with **low-quality, duplicate skills**. Staging + `promote approve` is a **human-in-the-loop quality gate** — slow but prevents irreversible skill debt.

### GC archive vs delete

Archiving to `facts_archive` with `reason_buckets` supports **postmortems** (“why did we lose this convention?”) without keeping noise in hot retrieval. `--force` on negatives is deliberately scary.

### Failure modes

| Failure | Impact | Mitigation |
|---------|--------|------------|
| Skipped eval in fork | Routing regressions ship | CI `stage-test.yml` |
| Aggressive GC | Useful facts archived | Dry-run default; protect negatives |
| Upstream child crash | `route_to_mcp` fails | `doctor --fix` codesign |
| Promotion without review | Skill library rot | Require approve step |

### Questions a PE should ask

1. Who runs **weekly digest** and acts on it?
2. Is **0.85 Recall@3** sufficient, or do you need custom golden sets per repo?
3. Which upstream MCPs are allowed in `upstream_mcp`? (Max 2 by design.)
4. What is the **promotion approval** owner for team skills?

## Trade-offs

- **Upstream** requires child process per server — macOS codesign matters (`doctor --fix`)
- **Promotion** is Cursor-skills-path biased today — other hosts may need path config
- **GC** heuristics can archive useful-but-unused facts — dry-run default mitigates

## Further reading

- [upstream/](../../agent-brain/src/upstream/)
- [promote.rs](../../agent-brain/src/promote.rs)
- [memory_gc.rs](../../agent-brain/src/memory_gc.rs)
- [eval.rs](../../agent-brain/src/eval.rs)
