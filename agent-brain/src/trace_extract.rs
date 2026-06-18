//! Mem0-inspired single-pass memory extraction from IDE tool traces (ADD-only).

use anyhow::Result;
use regex::Regex;
use serde::Serialize;

use crate::db::store::{content_hash, word_count, BrainStore, ToolTraceRow};
use crate::embed::Embedder;
use crate::tool_events;

#[derive(Debug, Clone, Serialize)]
pub struct TraceExtractReport {
    pub dry_run: bool,
    pub scanned: usize,
    pub extracted: usize,
    pub skipped: usize,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TraceExtractConfig {
    pub confidence: f64,
}

impl Default for TraceExtractConfig {
    fn default() -> Self {
        Self { confidence: 0.75 }
    }
}

pub fn run_trace_extract(
    store: &BrainStore,
    embedder: &Embedder,
    home: &std::path::Path,
    cfg: &TraceExtractConfig,
    dry_run: bool,
) -> Result<TraceExtractReport> {
    let since_ms = store
        .get_meta("last_trace_extract_ms")?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let _ = tool_events::ingest_hook_events_since(store, home, since_ms)?;

    let rows = store.list_pending_tool_traces(500)?;
    let mut extracted = 0usize;
    let mut skipped = 0usize;
    let mut topics = Vec::new();

    for row in &rows {
        let scope_key = row.scope_key.clone().or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|c| crate::config::find_repo_root(&c))
                .map(|p| p.display().to_string())
        });
        let Some(candidate) = candidate_from_trace(row) else {
            skipped += 1;
            if !dry_run {
                store.mark_trace_extracted(&row.id, None)?;
            }
            continue;
        };
        if word_count(&candidate.fact) > 50 {
            skipped += 1;
            if !dry_run {
                store.mark_trace_extracted(&row.id, None)?;
            }
            continue;
        }
        let hash = content_hash(&candidate.fact);
        if store.fact_exists_by_hash(&hash, "project", scope_key.as_deref())? {
            skipped += 1;
            if !dry_run {
                store.mark_trace_extracted(&row.id, None)?;
            }
            continue;
        }
        topics.push(candidate.topic.clone());
        if dry_run {
            extracted += 1;
            continue;
        }
        let embedding = embedder.embed_one(&format!("{} {}", candidate.topic, candidate.fact))?;
        let res = store.store_fact_full(
            &candidate.topic,
            &candidate.fact,
            "project",
            scope_key.as_deref(),
            cfg.confidence,
            "trace_extract",
            &hash,
            &embedding,
            "positive",
            candidate.apply_when.as_deref(),
            None,
        )?;
        store.mark_trace_extracted(&row.id, Some(&res.id))?;
        extracted += 1;
    }

    if !dry_run {
        let now = chrono::Utc::now().timestamp_millis();
        store.set_meta("last_trace_extract_ms", &now.to_string())?;
    }

    Ok(TraceExtractReport {
        dry_run,
        scanned: rows.len(),
        extracted,
        skipped,
        topics,
    })
}

struct TraceCandidate {
    topic: String,
    fact: String,
    apply_when: Option<String>,
}

fn candidate_from_trace(row: &ToolTraceRow) -> Option<TraceCandidate> {
    let detail = row.detail.as_deref().unwrap_or("").trim();
    let path = row.path.as_deref().unwrap_or("").trim();
    let tool = row.tool_name.to_ascii_lowercase();

    if tool.contains("store_memory") {
        return None;
    }

    if let Some(c) = match_package_manager(detail) {
        return Some(c);
    }
    if let Some(c) = match_cargo_add(detail) {
        return Some(c);
    }
    if let Some(c) = match_config_edit(path, detail) {
        return Some(c);
    }
    if let Some(c) = match_test_runner(detail) {
        return Some(c);
    }
    None
}

fn match_package_manager(detail: &str) -> Option<TraceCandidate> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(npm|pnpm|yarn)\s+(install|add)(?:\s+-D|\s+--save-dev)?\s+([\w@./-]+)")
            .expect("regex")
    });
    let caps = re.captures(detail)?;
    let pkg = caps.get(3)?.as_str();
    let topic = slug_topic(pkg);
    Some(TraceCandidate {
        topic: format!("deps-{topic}"),
        fact: format!("Project added dependency {pkg} via package manager"),
        apply_when: Some(r#"["phase:implementing"]"#.into()),
    })
}

fn match_cargo_add(detail: &str) -> Option<TraceCandidate> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)cargo\s+add\s+([\w-]+)").expect("regex"));
    let caps = re.captures(detail)?;
    let crate_name = caps.get(1)?.as_str();
    Some(TraceCandidate {
        topic: format!("deps-{crate_name}"),
        fact: format!("Project added Rust crate {crate_name} via cargo add"),
        apply_when: None,
    })
}

fn match_config_edit(path: &str, detail: &str) -> Option<TraceCandidate> {
    let lower = path.to_ascii_lowercase();
    if lower.contains("vitest.config") || lower.contains("vite.config") {
        return Some(TraceCandidate {
            topic: "testing-stack".into(),
            fact: "Vitest configuration was edited in this project".into(),
            apply_when: Some(r#"["phase:implementing"]"#.into()),
        });
    }
    if lower.ends_with("package.json") && (detail.contains("vitest") || detail.contains("jest")) {
        let runner = if detail.to_ascii_lowercase().contains("vitest") {
            "Vitest"
        } else {
            "Jest"
        };
        return Some(TraceCandidate {
            topic: "testing-stack".into(),
            fact: format!("package.json change references {runner} for tests"),
            apply_when: Some(r#"["phase:implementing"]"#.into()),
        });
    }
    None
}

fn match_test_runner(detail: &str) -> Option<TraceCandidate> {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("vitest") && (lower.contains("test") || lower.contains("run")) {
        return Some(TraceCandidate {
            topic: "testing-stack".into(),
            fact: "Shell trace shows Vitest used for running tests".into(),
            apply_when: Some(r#"["phase:implementing"]"#.into()),
        });
    }
    None
}

fn slug_topic(raw: &str) -> String {
    raw.split('@').next().unwrap_or(raw).replace('/', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_from_npm_install() {
        let row = ToolTraceRow {
            id: "1".into(),
            tool_name: "Shell".into(),
            path: None,
            detail: Some("npm install -D vitest".into()),
            scope_key: None,
        };
        let c = candidate_from_trace(&row).unwrap();
        assert_eq!(c.topic, "deps-vitest");
    }

    #[test]
    fn extracts_from_cargo_add() {
        let row = ToolTraceRow {
            id: "2".into(),
            tool_name: "Shell".into(),
            path: None,
            detail: Some("cargo add anyhow".into()),
            scope_key: None,
        };
        let c = candidate_from_trace(&row).unwrap();
        assert_eq!(c.topic, "deps-anyhow");
    }
}
