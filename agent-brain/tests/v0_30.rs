//! v0.30 — Knowledge Graph Phase 1: AST symbol storage in code_graph_nodes.

use std::path::Path;

use agent_brain::db::store::BrainStore;
use agent_brain::graphify::{AstCodeNodeRow, CodeGraphNodeRow};
use tempfile::TempDir;

fn open_store(dir: &TempDir) -> BrainStore {
    let db_path = dir.path().join("brain.db");
    BrainStore::open(&db_path).unwrap()
}

fn ast_nodes_for_root(repo_root: &str) -> Vec<AstCodeNodeRow> {
    vec![
        AstCodeNodeRow {
            repo_root: repo_root.to_string(),
            symbol_name: "handle_request".into(),
            symbol_kind: "function".into(),
            content: "fn handle_request() { Ok(()) }".into(),
            source_file: "src/handler.rs".into(),
            start_line: 10,
            end_line: 12,
            language: "Rust".into(),
            doc_comment: Some("Handles the incoming request.".into()),
        },
        AstCodeNodeRow {
            repo_root: repo_root.to_string(),
            symbol_name: "AppConfig".into(),
            symbol_kind: "struct".into(),
            content: "struct AppConfig { port: u16 }".into(),
            source_file: "src/config.rs".into(),
            start_line: 1,
            end_line: 3,
            language: "Rust".into(),
            doc_comment: None,
        },
    ]
}

fn count_ast_nodes(store: &BrainStore, repo_root: &str) -> usize {
    store
        .with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT COUNT(*) FROM code_graph_nodes WHERE repo_root = ?1 AND file_type LIKE 'ast:%'",
                )
                .unwrap();
            let n: i64 = stmt.query_row(rusqlite::params![repo_root], |r| r.get(0)).unwrap();
            Ok(n as usize)
        })
        .unwrap()
}

fn count_code_graph_nodes(store: &BrainStore, repo_root: &str) -> usize {
    store
        .with_conn(|conn| {
            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM code_graph_nodes WHERE repo_root = ?1")
                .unwrap();
            let n: i64 = stmt.query_row(rusqlite::params![repo_root], |r| r.get(0)).unwrap();
            Ok(n as usize)
        })
        .unwrap()
}

#[test]
fn migration_v15_adds_columns() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);

    store
        .with_conn(|conn| {
            let mut stmt = conn.prepare("PRAGMA table_info(code_graph_nodes)").unwrap();
            let cols: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            assert!(cols.contains(&"ast_symbol".into()), "ast_symbol column missing");
            assert!(cols.contains(&"embedding_id".into()), "embedding_id column missing");
            assert!(cols.contains(&"start_line".into()), "start_line column missing");
            assert!(cols.contains(&"end_line".into()), "end_line column missing");
            Ok(())
        })
        .unwrap();
}

#[test]
fn upsert_ast_code_nodes_inserts_new_nodes() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let repo = "/tmp/test-repo";

    let nodes = ast_nodes_for_root(repo);
    store.upsert_ast_code_nodes(repo, &nodes, 1000).unwrap();

    assert_eq!(count_ast_nodes(&store, repo), 2);
    assert_eq!(count_code_graph_nodes(&store, repo), 2);
}

#[test]
fn upsert_ast_code_nodes_updates_existing_by_graphify_id() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let repo = "/tmp/test-repo";

    let nodes = ast_nodes_for_root(repo);
    store.upsert_ast_code_nodes(repo, &nodes, 1000).unwrap();

    let updated = vec![AstCodeNodeRow {
        repo_root: repo.to_string(),
        symbol_name: "handle_request".into(),
        symbol_kind: "function".into(),
        content: "fn handle_request() -> Result<()> { Ok(()) }".into(),
        source_file: "src/handler.rs".into(),
        start_line: 10,
        end_line: 14,
        language: "Rust".into(),
        doc_comment: Some("Updated doc.".into()),
    }];
    store.upsert_ast_code_nodes(repo, &updated, 2000).unwrap();

    // handle_request updated in place, AppConfig unchanged
    assert_eq!(count_code_graph_nodes(&store, repo), 2);
    store
        .with_conn(|conn| {
            let mut stmt = conn
                .prepare("SELECT start_line, end_line, ast_symbol FROM code_graph_nodes WHERE graphify_id = ?1")
                .unwrap();
            let graphify_id = format!("ast:src/handler.rs:handle_request");
            let (start, end, ast_json): (i64, i64, String) = stmt
                .query_row(rusqlite::params![graphify_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .unwrap();
            assert_eq!(start, 10);
            assert_eq!(end, 14);
            assert!(ast_json.contains("Updated doc."), "ast_symbol should contain updated doc");
            Ok(())
        })
        .unwrap();
}

#[test]
fn ast_nodes_coexist_with_graphify_nodes() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let repo = "/tmp/test-repo";

    let graphify_nodes = vec![CodeGraphNodeRow {
        graphify_id: "module:main".into(),
        label: "main_module".into(),
        community_id: Some(1),
        is_god_node: false,
        source_file: Some("src/main.rs".into()),
        file_type: Some("rust".into()),
    }];
    store
        .replace_code_graph(repo, &graphify_nodes, &[], 100)
        .unwrap();

    let ast_nodes = ast_nodes_for_root(repo);
    store.upsert_ast_code_nodes(repo, &ast_nodes, 100).unwrap();

    assert_eq!(count_code_graph_nodes(&store, repo), 3);
    assert_eq!(count_ast_nodes(&store, repo), 2);
}

#[test]
fn replace_code_graph_clears_ast_nodes() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let repo = "/tmp/test-repo";

    let ast_nodes = ast_nodes_for_root(repo);
    store.upsert_ast_code_nodes(repo, &ast_nodes, 100).unwrap();
    assert_eq!(count_code_graph_nodes(&store, repo), 2);

    store
        .replace_code_graph(repo, &[], &[], 200)
        .unwrap();
    assert_eq!(count_code_graph_nodes(&store, repo), 0);
}

#[test]
fn count_code_graph_nodes_includes_ast_nodes() {
    let dir = TempDir::new().unwrap();
    let store = open_store(&dir);
    let repo = "/tmp/test-repo";

    assert_eq!(store.count_code_graph_nodes(Path::new(repo)).unwrap(), 0);

    let nodes = ast_nodes_for_root(repo);
    store.upsert_ast_code_nodes(repo, &nodes, 100).unwrap();

    assert_eq!(store.count_code_graph_nodes(Path::new(repo)).unwrap(), 2);
}
