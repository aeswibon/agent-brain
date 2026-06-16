# 3. Local-first storage

## Summary

All hot-path routing data lives in **`~/.agent_brain/data/brain.db`** (SQLite). Embeddings are stored per indexed row and per cached query hash. There is no separate vector database service.

## What we built

### SQLite as single source of truth

Tables include (evolving via schema migrations v1→v6):

| Area | Tables / mechanism |
|------|-------------------|
| Index | `indexed_items`, `items_fts` (FTS5) |
| Memory | `facts`, `facts_fts`, `facts_archive` |
| Routing observability | `retrieval_log`, `context_weights` |
| Conflicts | `conflict_log` |
| Promotion | `skill_staging` (v6) |
| Meta | `schema_version`, `meta` key-value |
| Query cache | `query_embeddings` |

**Content hashing:** Indexed files and facts use SHA-256 of normalized text to skip re-embed when unchanged.

### Embeddings

- **Model:** `AllMiniLML6V2` via `fastembed` (ONNX), configurable aliases (`mini`, `bge-small`, etc.)
- **Storage:** BLOB column on `indexed_items` and `facts`; unit-normalized vectors
- **Cache dir:** `~/.agent_brain/cache/fastembed` (via `FASTEMBED_CACHE_DIR` / `AGENT_BRAIN_HOME`) — not the project working directory

**Tests** use `Embedder::deterministic()` (hash-based unit vectors) to avoid ONNX downloads and lock contention in parallel `cargo test`.

### Scopes

| Scope | `scope_key` | Use |
|-------|-------------|-----|
| `global` | NULL | Machine-wide conventions |
| `project` | repo root path | Repo-specific stack, test runner |
| `package` | package name | ECC bundle isolation |

Route scoring boosts items whose `scope_key` matches the current repo root.

## Why SQLite + FTS5 + embeddings (hybrid)

**Not vectors-only:** Skill names (folder names) often do not match user language (“review the PR” vs `requesting-code-review`). Pure cosine similarity on short skill text misses lexical matches.

**Not BM25-only:** Synonyms and paraphrases need semantic similarity.

**Hybrid pipeline (current):**

1. FTS5 BM25 prefilter (strict AND terms, loose OR fallback)
2. Cosine similarity on query embedding vs item embeddings
3. Lexical term overlap between query and topic+text
4. Phase boost, scope boost, context weight feedback

See [04-turn-routing-and-retrieval.md](04-turn-routing-and-retrieval.md).

## Alternatives considered

### Separate `vectors.bin` flat file (master spec)

Early spec described `vectors.bin` aligned by row ID alongside SQLite.

**Current code:** Embeddings live in SQLite BLOBs. Simpler backup (one file), fewer sync corruption modes. A separate binary index may return if embedding matrix size dominates DB size.

### External vector DB (Qdrant, LanceDB, sqlite-vec)

**Pros:** ANN scale, dedicated tooling.

**Cons:** Extra process or extension; sync story harder; typical dev machines have hundreds–low thousands of skills, not millions of chunks. SQLite brute-force dot products on hundreds of candidates is fast enough after BM25 prefilter.

### JSON `atomic_facts.json` hot path

**Rejected:** No FTS, no transactions, poor concurrent write story. JSON remains **export-only** (`export_memory`).

### PostgreSQL / server DB

**Rejected:** Violates local-first, complicates sync and install.

## Schema evolution

Migrations in `db/migrations.rs` bump `schema_version` in place. Notable jumps:

- **v4:** `context_weights` for useful/useless feedback
- **v5:** `retrieval_log`, upstream-related meta
- **v6:** `skill_staging`, `facts_archive` for operator loop

**Rationale:** One binary upgrades user DBs on open; no manual migration CLI for normal users.

## Trade-offs

- **Single-writer SQLite:** Write queue serializes imports; heavy parallel writes not supported.
- **FTS tokenization:** English-centric; CJK or code-heavy queries may need future tokenizer tweaks.
- **Re-index on model change:** `embedding_model` change in config flags need to re-bootstrap embeddings.

## For senior engineers and principal architects

### Why one file (`brain.db`) instead of a storage micro-stack

A PE reviewing local-first systems often asks: “Why not Postgres + Qdrant?” Our answer is **operational surface area**:

- **Backup** = copy one file (or export bundle)
- **Sync** = point-in-time bundle, not WAL replication
- **Corruption recovery** = re-index from disk skills + re-import exports

The brain DB is a **derived cache** of filesystem skills plus user-authored facts. Losing it is painful but recoverable via `agent-brain index` and sync pull. That property would break if the DB were the only copy of skill content.

### Embedding storage in-row (BLOB) vs sidecar

| Approach | Pros | Cons |
|----------|------|------|
| BLOB in SQLite (chosen) | Atomic backup, simple migrations | DB grows with model changes |
| `vectors.bin` sidecar | Faster mmap scan at huge N | Two-file sync, drift risk |
| External ANN service | Scale | Violates local-first |

We are in the **hundreds to low thousands** of indexed rows regime. Brute-force dot product on BM25-filtered candidates is O(candidates × dim), not O(entire index). Revisit at ~10k+ active rows or if p95 scoring exceeds budget.

### Scope model rationale

`scope` + `scope_key` exist because **global ECC skills** and **repo-specific CLAUDE.md** compete in the same index. Without scope boost, a generic global rule drowns a project-specific fact. This mirrors multi-tenant row-level security in SaaS — except tenants are `global`, `project`, and `package`, not user IDs.

### Schema migration policy

Migrations run **on open**, in-process, forward-only. We do not ship a separate migration CLI because the target user is an individual developer, not a DBA team. PE implication: **downgrades are unsupported**; keep export bundles before risky upgrades.

### Failure modes

| Failure | Behavior | Recovery |
|---------|----------|----------|
| `SQLITE_BUSY` | Retries in store layer | Reduce parallel writers; avoid live Dropbox on DB |
| Model dimension mismatch | Re-embed on bootstrap | Run `index` after model change |
| FTS query empty | Loose OR fallback | Check `retrieval.rs` stopword stripping |
| Corrupt DB | Open may fail | Restore from git/cloud bundle |

### Questions a PE should ask

1. Where does **PII** land? (Project paths in facts, session digests — treat DB as sensitive.)
2. Is **English FTS** sufficient for your skill library language mix?
3. Do you need **encryption at rest** beyond cloud sync bundle encryption? (Not built-in for local DB.)
4. What is your **retention policy** for `retrieval_log` and archived facts?

## Further reading

- [05-memory-model.md](05-memory-model.md) — facts table semantics
- [db/migrations.rs](../../agent-brain/src/db/migrations.rs)
- [embed.rs](../../agent-brain/src/embed.rs)
