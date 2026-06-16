# 5. Memory model

## Summary

Memory in agent-brain is **structured facts**, not chat transcripts. Each fact is a short, scoped statement the router can surface on future similar tasks via `route_task` → `relevant_memory`.

## What we built

### write path: `store_memory`

MCP tool (and CLI flows) accept:

- `topic` — short label (e.g. `testing-framework`)
- `fact` — max ~50 words, enforced
- `scope` / `scope_key` — global vs project
- `confidence`, `polarity`, optional `apply_when`

Writes go through the **write queue** to avoid races with sync import.

### Deduplication and supersession

- **Content hash** of normalized fact text prevents duplicate inserts in same scope.
- **Same topic** in same scope: newer fact **supersedes** older (`superseded_by`), with `conflict_log` for audit.

**Why:** Agents repeat “use Vitest” many times; store should converge, not grow unbounded duplicates.

### Negative memory

`polarity: "negative"` marks constraints (“never use Jest”). These:

- Surface in `must_apply` / high-priority memory slots when relevant
- Are **protected** from memory GC unless `--force`

**Rationale:** Negative constraints are safety-critical; auto-archive could reintroduce bad patterns.

### `apply_when` conditions

Parsed in `intelligence/apply_when.rs`. Examples:

- `phase:reviewing`
- `path:**/*.rs`
- `tag:rust`

When conditions match `MatchContext` (phase, open files, repo, tags), memory gets a score boost; non-matching conditioned facts are down-weighted.

**Why not free-text only:** Structured conditions reduce false positives (“use pnpm” applying during unrelated Python work).

### Context feedback loop

`report_context_useful` MCP tool updates `context_weights`:

- Useful → weight up
- Useless → weight down

Low-weight, stale items become **GC candidates** (see operator loop doc).

### Retrieval as memory type

Facts are embedded and FTS-indexed like skills. They compete in hybrid scoring but are assembled in **pass 2** of `build_route_response` so skills keep budget priority.

## Alternatives considered

### Store full conversation transcripts

**Rejected for hot path:** Token-expensive, privacy-heavy, poor precision. **Session digest** (separate pipeline) extracts user messages into optional facts — not raw transcript storage.

### Vector-only memory (no topic/scope)

**Rejected:** Cannot dedupe by topic, scope conflicts, or apply `apply_when`.

### User-editable YAML facts on disk

**Considered:** Git-friendly, human-readable.

**Current:** SQLite is canonical; `export_memory` / sync bundles provide portability. YAML hot path would duplicate index and FTS.

### Unlimited fact length

**Rejected:** Memory must fit router budget and agent context; long prose belongs in skills or docs.

### Auto-capture every assistant message

**Deferred / off by default:** High noise; `auto_capture_enabled` exists but human/agent explicit `store_memory` is the intended durable path.

## Trade-offs

- **50-word cap** requires agents to compress conventions — good for routing, bad for dumping paragraphs.
- **Supersession by topic** assumes one active truth per topic per scope — ambiguous topics need distinct topic names.
- **GC** can archive stale session digests; protected negatives need operator trust in `--force`.

## For senior engineers and principal architects

### Memory is a control plane, not a knowledge base

Facts are sized for **routing decisions** (“use Vitest”, “never Jest”), not documentation. If your org wants a wiki, use Confluence — agent-brain memory is **executable policy** surfaced when similar tasks recur.

### Negative polarity as safety invariant

`polarity: negative` facts are **GC-protected** because false archival is worse than false retention: the agent might reintroduce a banned pattern (wrong test runner, deprecated API). This mirrors **deny rules** in policy engines — removes are harder than adds.

### `apply_when` vs dumping everything global

Unconditioned global facts have high **false-positive rate** on unrelated tasks. `apply_when` is our lightweight **policy attachment**:

```text
fact + conditions(phase, path glob, tag) → score boost only when context matches
```

Full trigger DSL per skill was deferred; memory conditions prove the pattern. Skill-level triggers would duplicate index text unless we unify “activation” metadata.

### Write path serialization

`store_memory` goes through the write queue because **sync import** and **agent store** racing caused lost supersession and duplicate topics in early testing. PE lesson: even “low QPS” writes need serialization when imports are bursty.

### Conflict surfacing

`scope_conflict_warnings` exposes **global vs project disagreement** on the same topic without silently picking a winner. The agent (or human) resolves ambiguity — the router does not merge semantics.

### Failure modes

| Failure | Symptom | Mitigation |
|---------|---------|------------|
| Topic collision | New fact supersedes unrelated old fact | Use distinct topic names |
| Session digest noise | Irrelevant memory in routes | GC stale digests; tighten ingest |
| Over-confident fact | Wrong convention persists | `delete_memory`, negative fact, lower confidence |
| apply_when too narrow | Fact never surfaces | Broaden conditions or remove |

### Questions a PE should ask

1. Who **owns** memory hygiene? (Without an operator, GC never runs.)
2. Do you need **audit trail** for compliance? (`conflict_log`, `facts_archive` help partially.)
3. Are **50-word facts** sufficient for your conventions, or do teams dump paragraphs anyway?
4. Should **session ingest** be on by default for your privacy posture?

## Further reading

- [09-upstream-and-operator-loop.md](09-upstream-and-operator-loop.md) — memory GC
- [intelligence/apply_when.rs](../../agent-brain/src/intelligence/apply_when.rs)
- [db/store.rs](../../agent-brain/src/db/store.rs) — `store_fact`, `record_context_feedback`
