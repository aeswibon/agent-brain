//! Retrieval eval harness for CI gates (Recall@3).

use std::sync::Arc;

use anyhow::{bail, Result};

use crate::db::store::{content_hash, BrainStore};
use crate::embed::deterministic_embedding;
use crate::engine::Engine;
use crate::types::RouteLimits;

pub const RECALL_AT_3_THRESHOLD: f64 = 0.85;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvalReport {
    pub cases: usize,
    pub passed: usize,
    pub recall_at_3: f64,
    pub threshold: f64,
    pub failures: Vec<EvalFailure>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvalFailure {
    pub query: String,
    pub expected_topics: Vec<String>,
    pub got_topics: Vec<String>,
}

struct GoldenCase {
    query: &'static str,
    fact: &'static str,
    topic: &'static str,
}

const GOLDEN: &[GoldenCase] = &[
    GoldenCase {
        query: "configure vitest for react testing",
        fact: "Do not use Jest for this project; prefer Vitest",
        topic: "testing-framework",
    },
    GoldenCase {
        query: "postgres connection pool settings",
        fact: "Use PgBouncer in transaction mode for serverless Postgres",
        topic: "postgres-pooling",
    },
    GoldenCase {
        query: "rust error handling patterns",
        fact: "Prefer anyhow::Result in binaries and thiserror in libraries",
        topic: "rust-errors",
    },
    GoldenCase {
        query: "mcp server stdio transport",
        fact: "agent-brain MCP servers use stdio transport with rmcp",
        topic: "mcp-transport",
    },
];

pub fn run_ci_eval(engine: &Engine) -> Result<EvalReport> {
    seed_golden_facts(&engine.store)?;

    let limits = RouteLimits {
        agents: 0,
        skills: 0,
        rules: 0,
        memory: 5,
    };

    let mut passed = 0usize;
    let mut failures = Vec::new();

    for case in GOLDEN {
        let resp = engine.route_task(
            case.query,
            None,
            &[],
            500,
            limits,
            Some("implementing"),
        )?;

        let got_topics: Vec<String> = resp
            .relevant_memory
            .iter()
            .take(3)
            .map(|m| m.topic.clone())
            .collect();

        if got_topics.iter().any(|t| t == case.topic) {
            passed += 1;
        } else {
            failures.push(EvalFailure {
                query: case.query.to_string(),
                expected_topics: vec![case.topic.to_string()],
                got_topics,
            });
        }
    }

    let cases = GOLDEN.len();
    let recall_at_3 = if cases == 0 {
        1.0
    } else {
        passed as f64 / cases as f64
    };

    Ok(EvalReport {
        cases,
        passed,
        recall_at_3,
        threshold: RECALL_AT_3_THRESHOLD,
        failures,
    })
}

pub fn assert_ci_gate(report: &EvalReport) -> Result<()> {
    if report.recall_at_3 >= RECALL_AT_3_THRESHOLD {
        return Ok(());
    }
    bail!(
        "Recall@3 {:.2} below threshold {:.2} ({} / {} passed)",
        report.recall_at_3,
        RECALL_AT_3_THRESHOLD,
        report.passed,
        report.cases
    );
}

fn seed_golden_facts(store: &Arc<BrainStore>) -> Result<()> {
    for case in GOLDEN {
        let emb = deterministic_embedding(case.fact);
        let hash = content_hash(case.fact);
        store.store_fact(
            case.topic,
            case.fact,
            "global",
            None,
            0.95,
            "eval",
            &hash,
            &emb,
            "positive",
        )?;
    }
    store.bump_index_version()?;
    Ok(())
}
