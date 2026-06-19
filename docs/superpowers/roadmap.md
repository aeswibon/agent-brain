# agent-brain Roadmap

## Standalone Features
*Features required to make `agent-brain` the ultimate independent memory engine for any AI tool.*

- [ ] **Universal API Server:** Expose a REST/gRPC API alongside MCP so non-IDE tools (like LangChain or custom scripts) can query the memory graph.
- [ ] **Pluggable Embedders:** Support external embedding APIs (OpenAI `text-embedding-3-small`, Voyage) alongside the local ONNX models for users who prioritize quality over pure local execution.
- [ ] **CLI RAG Client:** Build `agent-brain query "How does X work?"` to allow developers to perform semantic codebase searches instantly from the terminal.
- [ ] **Data Export/Import:** Support exporting the SQLite graph to standard JSON/CSV formats for analytics or migration.

## Integrated Features
*Features required to make `agent-brain` an unstoppable node in the Autonomic ecosystem.*

- [ ] **Native `agent-spine` Hook:** Build the `BrainRouter` directly into `agent-spine` execution nodes so agents automatically receive enriched context without explicitly requesting it.
- [ ] **Background Distillation:** Expose hooks for `agent-heart` to trigger nightly cron jobs that compress and deduplicate `agent-brain` vectors.
- [ ] **Omnichannel Ingestion:** Automatically ingest Slack ChatOps logs from `agent-mouth` into the memory graph.
