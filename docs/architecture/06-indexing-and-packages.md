# 6. Indexing and skill packages

## Summary

agent-brain does not author skills — it **discovers** them from configured filesystem roots, hashes content, embeds text, and upserts into `indexed_items`. **Packages** (`agent-brain add owner/repo`) install curated bundles (e.g. ECC) into `~/.agent_brain/packages/`.

## What we built

### Index roots (`config::default_index_roots`)

| Source | Path pattern |
|--------|----------------|
| Router-local | `~/.agent_brain/{rules,skills,agents}` |
| Packages | `~/.agent_brain/packages/*/` |
| Cursor | `~/.cursor/skills-cursor`, `~/.cursor/skills`, plugin agents |
| Claude / Codex | `~/.claude/{skills,agents}`, `~/.codex/{skills,agents}` |
| Project | `.cursor/rules`, `.cursor/agents`, `CLAUDE.md`, `AGENTS.md`, `.cursorrules` |

Walk is read-only; source files are never modified.

### File type detection (`index.rs`)

| Pattern | Item type |
|---------|-----------|
| `SKILL.md` | skill (topic = parent folder name) |
| `commands/*`, `agents/*` | skill or agent |
| `.mdc`, `CLAUDE.md`, rules | rule |

**Skipped paths:** `node_modules`, `target`, `.git`, `graphify-out/` (graphify skill output — not created by agent-brain).

### Skill text extraction

From SKILL.md:

1. Folder name (topic)
2. YAML `description` and `name` from frontmatter
3. **"When to activate / use"** section (if present) — highest-signal routing text
4. First ~20 body lines as fallback

**Why frontmatter matters:** Users query in natural language; descriptions contain “pull request”, “Vitest”, etc., while folder names may be opaque (`babysit`, `ecc/github-ops`).

**Why activation sections matter:** ECC skills often bury intent in prose. Extracting activation headings indexes the same phrases users type in chat — this is the main lever for routing accuracy without per-query hacks (see [12](12-routing-accuracy.md)).

### Bootstrap

Triggered on `serve` startup (background) and `agent-brain index`:

- `Engine::bootstrap` → `index::sync_index`
- Interval gated by `bootstrap_interval_secs` and `last_bootstrap_unix` meta
- Bumps `index_version` on changes → invalidates turn cache

### Packages

`packages/mod.rs` + `agent-brain add`:

- Clone or install skill repos into `~/.agent_brain/packages/<name>/`
- Index roots include package subdirectory
- `scope: package` + `scope_key: package name` for isolation

## Alternatives considered

### Watch filesystem with inotify / FSEvents

**Deferred:** Cross-platform complexity; bootstrap interval is “good enough” for skill edits. Manual `index` for immediate refresh.

### Central skill registry API

**Rejected:** Couples install to network; packages use GitHub releases/git clone instead.

### Duplicate skills into `~/.agent_brain/skills`

**Rejected:** Source of truth stays upstream paths; only metadata + embeddings cached in DB.

### Index full 10k-token skill bodies

**Rejected:** Embedding model input limits; retrieval uses summaries; agent reads full file after route.

### Single global index without project scope

**Rejected:** Monorepos need project rules (`CLAUDE.md`) boosted over global noise.

## Trade-offs

- **Stale index** until next bootstrap after skill edit (typically ≤1h with default interval).
- **Duplicate skill names** across paths: deduped in route response by topic lowercase, not by path — last scorer wins.
- **Plugin agent paths** discovered via glob — unusual layouts may need local copies under `~/.agent_brain/agents`.

## For senior engineers and principal architects

### Index as derived view (CQRS-ish)

Filesystem skills are the **source of truth**; `indexed_items` is a **read model** optimized for hybrid search. We never edit upstream SKILL.md during index — avoids fork drift and respects package updates (`agent-brain add`).

Implication for teams: **skill authoring quality = routing quality**. No amount of router tuning fixes skills with empty descriptions and cryptic folder names.

### Bootstrap interval vs watch

We chose **periodic bootstrap** over filesystem watchers because:

- Cross-platform watch APIs differ (FSEvents, inotify, ReadDirectoryChangesW)
- IDE saves burst during agent edits — watch storms re-embed constantly
- `index_version` bump invalidates turn cache — aggressive watch harms latency

Manual `agent-brain index` remains the **force refresh** for skill authors. PEs standardizing skill packs should run index in CI after install.

### Package scope isolation

ECC-style packages use `scope: package` so a bundled skill does not pretend to be global project truth. This prevents **cross-package topic collisions** from polluting repo-scoped boosts.

### graphify-out skip

`graphify-out/` is skipped because it is **skill output**, not authored skills — indexing it would add noise and confuse retrieval. agent-brain did not create that directory; the graphify skill did.

### Failure modes

| Failure | Effect | Fix |
|---------|--------|-----|
| Skill edited, route stale | Old embedding wins until bootstrap | `agent-brain index` |
| Duplicate topic names | Unpredictable winner in route dedup | Rename folder or namespace packages |
| Missing index root | Skills invisible | `doctor`, check `config.yaml` roots |
| Huge skill body | Truncated at 800 chars indexed | Put routing phrases in frontmatter / activation |

### Questions a PE should ask

1. Where do **authoritative skills** live for your org? (Cursor global, package, monorepo `.cursor/rules`?)
2. Do you enforce **skill metadata standards** (description, activation section)?
3. How often must index be **fresh** for your workflow? (Tune `bootstrap_interval_secs`.)
4. Will multiple packages create **topic collisions**? (Naming convention needed.)

## Further reading

- [index.rs](../../agent-brain/src/index.rs)
- [config.rs](../../agent-brain/src/config.rs) — `default_index_roots`
- [04-turn-routing-and-retrieval.md](04-turn-routing-and-retrieval.md)
