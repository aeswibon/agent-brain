use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use uuid::Uuid;

use crate::db::store::BrainStore;

use super::repos::touch_ingest;
use super::types::{
    node_id_str, GraphJson, GraphifyAnalysis, GraphJsonEdge, GraphJsonNode,
};

pub fn ingest_repo(store: &BrainStore, home: &Path, repo_root: &Path) -> Result<IngestReport> {
    let graph_path = repo_root.join("graphify-out").join("graph.json");
    if !graph_path.is_file() {
        bail!(
            "no graph at {} — run graphify in the repo first",
            graph_path.display()
        );
    }
    let raw = fs::read_to_string(&graph_path)
        .with_context(|| format!("read {}", graph_path.display()))?;
    let graph: GraphJson = serde_json::from_str(&raw).context("parse graph.json")?;
    let gods = load_god_nodes(repo_root);
    let now = chrono::Utc::now().timestamp();
    let repo_str = repo_root.display().to_string();

    let nodes = normalize_nodes(&graph.nodes, &gods);
    let edges = normalize_edges(&graph.links, &graph.edges);

    store.replace_code_graph(&repo_str, &nodes, &edges, now)?;
    store.bump_index_version()?;

    let graph_mtime = graph_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    touch_ingest(home, repo_root, graph_mtime)?;

    Ok(IngestReport {
        nodes: nodes.len(),
        edges: edges.len(),
        god_nodes: gods.len(),
    })
}

#[derive(Debug, Clone)]
pub struct IngestReport {
    pub nodes: usize,
    pub edges: usize,
    pub god_nodes: usize,
}

fn load_god_nodes(repo_root: &Path) -> Vec<String> {
    let path = repo_root.join("graphify-out").join(".graphify_analysis.json");
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<GraphifyAnalysis>(&raw)
        .map(|a| a.gods)
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct CodeGraphNodeRow {
    pub graphify_id: String,
    pub label: String,
    pub community_id: Option<i64>,
    pub is_god_node: bool,
    pub source_file: Option<String>,
    pub file_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CodeGraphEdgeRow {
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
    pub confidence: Option<String>,
    pub confidence_score: Option<f64>,
}

fn normalize_nodes(nodes: &[GraphJsonNode], gods: &[String]) -> Vec<CodeGraphNodeRow> {
    nodes
        .iter()
        .map(|n| {
            let graphify_id = node_id_str(&n.id);
            let label = n
                .label
                .clone()
                .unwrap_or_else(|| graphify_id.clone());
            let community_id = n.community.or(n.community_id);
            let is_god_node = gods.iter().any(|g| g == &label || g == &graphify_id);
            CodeGraphNodeRow {
                graphify_id,
                label,
                community_id,
                is_god_node,
                source_file: n.source_file.clone(),
                file_type: n.file_type.clone(),
            }
        })
        .collect()
}

fn normalize_edges(links: &[GraphJsonEdge], edges: &[GraphJsonEdge]) -> Vec<CodeGraphEdgeRow> {
    let mut out = Vec::new();
    for e in links.iter().chain(edges.iter()) {
        out.push(edge_row(e));
    }
    out
}

fn edge_row(e: &GraphJsonEdge) -> CodeGraphEdgeRow {
    CodeGraphEdgeRow {
        source_id: node_id_str(&e.source),
        target_id: node_id_str(&e.target),
        relation: e
            .relation
            .clone()
            .or_else(|| e.key.clone())
            .unwrap_or_else(|| "related".into()),
        confidence: e.confidence.clone(),
        confidence_score: e.confidence_score,
    }
}

impl BrainStore {
    pub fn replace_code_graph(
        &self,
        repo_root: &str,
        nodes: &[CodeGraphNodeRow],
        edges: &[CodeGraphEdgeRow],
        ingested_at: i64,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM code_graph_edges WHERE repo_root = ?1",
                [repo_root],
            )?;
            conn.execute(
                "DELETE FROM code_graph_nodes WHERE repo_root = ?1",
                [repo_root],
            )?;
            for n in nodes {
                let id = format!("{repo_root}:{}", n.graphify_id);
                conn.execute(
                    r#"
                    INSERT INTO code_graph_nodes (
                        id, repo_root, graphify_id, label, community_id, is_god_node,
                        source_file, file_type, ingested_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    "#,
                    rusqlite::params![
                        id,
                        repo_root,
                        n.graphify_id,
                        n.label,
                        n.community_id,
                        n.is_god_node as i64,
                        n.source_file,
                        n.file_type,
                        ingested_at,
                    ],
                )?;
            }
            for e in edges {
                let id = Uuid::new_v4().to_string();
                conn.execute(
                    r#"
                    INSERT INTO code_graph_edges (
                        id, repo_root, source_id, target_id, relation,
                        confidence, confidence_score, ingested_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#,
                    rusqlite::params![
                        id,
                        repo_root,
                        e.source_id,
                        e.target_id,
                        e.relation,
                        e.confidence,
                        e.confidence_score,
                        ingested_at,
                    ],
                )?;
            }
            Ok(())
        })
    }

    pub fn count_code_graph_nodes(&self, repo_root: &Path) -> Result<usize> {
        let repo_str = repo_root.display().to_string();
        self.with_conn(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM code_graph_nodes WHERE repo_root = ?1",
                [repo_str],
                |r| r.get(0),
            )?;
            Ok(n as usize)
        })
    }

    pub fn list_god_nodes(&self, repo_root: &Path, limit: usize) -> Result<Vec<String>> {
        let repo_str = repo_root.display().to_string();
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT label FROM code_graph_nodes WHERE repo_root = ?1 AND is_god_node = 1 LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![repo_str, limit as i64], |r| {
                r.get::<_, String>(0)
            })?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
    }

    pub fn search_code_graph_labels(
        &self,
        repo_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<super::types::CodeContextNode>> {
        let repo_str = repo_root.display().to_string();
        let pattern = format!("%{}%", query.split_whitespace().next().unwrap_or(query));
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT n.label, e.relation, t.label
                FROM code_graph_nodes n
                LEFT JOIN code_graph_edges e ON e.repo_root = n.repo_root AND e.source_id = n.graphify_id
                LEFT JOIN code_graph_nodes t ON t.repo_root = n.repo_root AND t.graphify_id = e.target_id
                WHERE n.repo_root = ?1 AND n.label LIKE ?2
                LIMIT ?3
                "#,
            )?;
            let rows = stmt.query_map(rusqlite::params![repo_str, pattern, limit as i64], |r| {
                Ok(super::types::CodeContextNode {
                    label: r.get(0)?,
                    relation: r.get(1)?,
                    target: r.get(2)?,
                })
            })?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
    }

    pub fn last_code_graph_ingest(&self, repo_root: &Path) -> Result<Option<i64>> {
        let repo_str = repo_root.display().to_string();
        self.with_conn(|conn| {
            let v: Option<i64> = conn
                .query_row(
                    "SELECT MAX(ingested_at) FROM code_graph_nodes WHERE repo_root = ?1",
                    [repo_str],
                    |r| r.get(0),
                )
                .ok();
            Ok(v)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_networkx_style_graph() {
        let raw = r#"{
            "nodes": [{"id": "Auth", "label": "AuthModule"}],
            "links": [{"source": "Auth", "target": "Db", "relation": "calls"}]
        }"#;
        let g: GraphJson = serde_json::from_str(raw).unwrap();
        let nodes = normalize_nodes(&g.nodes, &["AuthModule".into()]);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].is_god_node);
        let edges = normalize_edges(&g.links, &g.edges);
        assert_eq!(edges[0].relation, "calls");
    }
}
